use alloy_rlp::RlpEncodable;
use edr_eth::{
    block::{self, BlobGas, PartialHeader},
    chain_spec::EthHeaderConstants,
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams, ForkBaseFeeParams},
    env::{BlobExcessGasAndPrice, BlockEnv},
    result::{HaltReason, InvalidTransaction},
    U256,
};
use edr_evm::{
    chain_spec::{BlockEnvConstructor, EvmSpec},
    transaction::{TransactionError, TransactionValidation},
    RemoteBlockConversionError,
};
use edr_generic::GenericChainSpec;
use edr_napi_core::{
    napi,
    spec::{marshal_response_data, Response, SyncNapiSpec},
};
use edr_provider::{time::TimeSinceEpoch, ProviderSpec, TransactionFailureReason};
use edr_rpc_eth::{jsonrpc, spec::RpcSpec};
use revm::{
    handler::register::HandleRegisters,
    optimism::{OptimismHaltReason, OptimismInvalidTransaction, OptimismSpecId},
    primitives::ChainSpec,
    Database, EvmHandler,
};
use revm_optimism::{OptimismSpecId, OptimismWiring};
use serde::{de::DeserializeOwned, Serialize};

use crate::{eip2718::TypedEnvelope, hardfork, receipt, rpc, transaction};

/// Chain specification for the Ethereum JSON-RPC API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct OptimismChainSpec;

impl RpcSpec for OptimismChainSpec {
    type ExecutionReceipt<Log> = TypedEnvelope<receipt::Execution<Log>>;
    type RpcBlock<Data> = edr_rpc_eth::Block<Data> where Data: Default + DeserializeOwned + Serialize;
    type RpcCallRequest = edr_rpc_eth::CallRequest;
    type RpcReceipt = rpc::BlockReceipt;
    type RpcTransaction = rpc::Transaction;
    type RpcTransactionRequest = edr_rpc_eth::TransactionRequest;
}

impl revm::primitives::ChainSpec for OptimismChainSpec {
    type ChainContext = revm_optimism::Context;
    type Block = edr_eth::env::BlockEnv;
    type Transaction = transaction::Signed;
    type Hardfork = OptimismSpecId;
    type HaltReason = OptimismHaltReason;
}

impl BlockEnvConstructor<OptimismChainSpec, PartialHeader> for revm::primitives::BlockEnv {
    fn new_block_env(header: &PartialHeader, hardfork: OptimismSpecId) -> Self {
        BlockEnv {
            number: U256::from(header.number),
            coinbase: header.beneficiary,
            timestamp: U256::from(header.timestamp),
            difficulty: header.difficulty,
            basefee: header.base_fee.unwrap_or(U256::ZERO),
            gas_limit: U256::from(header.gas_limit),
            prevrandao: if hardfork >= OptimismSpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header
                .blob_gas
                .as_ref()
                .map(|BlobGas { excess_gas, .. }| BlobExcessGasAndPrice::new(*excess_gas)),
        }
    }
}

impl BlockEnvConstructor<OptimismSpecId, block::Header> for revm::primitives::BlockEnv {
    fn new_block_env(header: &block::Header, hardfork: OptimismSpecId) -> Self {
        BlockEnv {
            number: U256::from(header.number),
            coinbase: header.beneficiary,
            timestamp: U256::from(header.timestamp),
            difficulty: header.difficulty,
            basefee: header.base_fee_per_gas.unwrap_or(U256::ZERO),
            gas_limit: U256::from(header.gas_limit),
            prevrandao: if hardfork >= OptimismSpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header
                .blob_gas
                .as_ref()
                .map(|BlobGas { excess_gas, .. }| BlobExcessGasAndPrice::new(*excess_gas)),
        }
    }
}

/// EVM wiring for Optimism chains.
pub struct Wiring<ChainSpecT: ChainSpec, DatabaseT: Database, ExternalContextT> {
    _phantom: PhantomData<(ChainSpecT, DatabaseT, ExternalContextT)>,
}

impl<ChainSpecT, DatabaseT, ExternalContextT> edr_eth::chain_spec::EvmWiring
    for Wiring<ChainSpecT, DatabaseT, ExternalContextT>
where
    ChainSpecT: ChainSpec + revm_optimism::OptimismChainSpec,
    DatabaseT: Database,
{
    type ChainSpec = ChainSpecT;
    type ExternalContext = ExternalContextT;
    type Database = DatabaseT;
}

impl<ChainSpecT, DatabaseT, ExternalContextT> revm::EvmWiring
    for Wiring<ChainSpecT, DatabaseT, ExternalContextT>
where
    ChainSpecT: OptimismWiring,
    DatabaseT: Database,
{
    fn handler<'evm>(hardfork: Self::Hardfork) -> revm::EvmHandler<'evm, Self> {
        let mut handler = EvmHandler::mainnet_with_spec(hardfork);

        handler.append_handler_register(HandleRegisters::Plain(
            revm_optimism::optimism_handle_register::<ChainSpecT, DB, EXT>,
        ));

        handler
    }
}

impl EvmSpec for OptimismChainSpec {
    type EvmWiring<DatabaseT: Database, ExternalContexT> = Wiring<Self, DatabaseT, ExternalContexT>;

    type ReceiptBuilder = receipt::execution::Builder;
    type RpcBlockConversionError = RemoteBlockConversionError<Self>;
    type RpcReceiptConversionError = rpc::receipt::ConversionError;
    type RpcTransactionConversionError = rpc::transaction::ConversionError;

    fn cast_transaction_error<BlockchainErrorT, StateErrorT>(
        error: <Self::Transaction as TransactionValidation>::ValidationError,
    ) -> TransactionError<Self, BlockchainErrorT, StateErrorT> {
        match error {
            OptimismInvalidTransaction::Base(InvalidTransaction::LackOfFundForMaxFee {
                fee,
                balance,
            }) => TransactionError::LackOfFundForMaxFee { fee, balance },
            remainder => TransactionError::InvalidTransaction(remainder),
        }
    }

    fn chain_hardfork_activations(
        chain_id: u64,
    ) -> Option<&'static edr_evm::hardfork::Activations<Self>> {
        hardfork::chain_hardfork_activations(chain_id)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        hardfork::chain_name(chain_id)
    }
}

impl EthHeaderConstants for OptimismChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<OptimismSpecId> =
        BaseFeeParams::Variable(ForkBaseFeeParams::new(&[
            (OptimismSpecId::LONDON, ConstantBaseFeeParams::new(50, 6)),
            (OptimismSpecId::CANYON, ConstantBaseFeeParams::new(250, 6)),
        ]));

    const MIN_ETHASH_DIFFICULTY: u64 = 0;
}

impl SyncNapiSpec for OptimismChainSpec {
    const CHAIN_TYPE: &'static str = "Optimism";

    fn cast_response(
        response: Result<
            edr_provider::ResponseWithTraces<OptimismHaltReason>,
            edr_provider::ProviderError<Self>,
        >,
    ) -> napi::Result<edr_napi_core::spec::Response<HaltReason>> {
        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

        marshal_response_data(response).map(|data| Response {
            solidity_trace: None,
            data,
            traces: Vec::new(),
        })
    }
}

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for OptimismChainSpec {
    type PooledTransaction = transaction::Pooled;
    type TransactionRequest = transaction::Request;

    fn cast_halt_reason(
        reason: OptimismHaltReason,
    ) -> TransactionFailureReason<OptimismHaltReason> {
        match reason {
            OptimismHaltReason::Base(reason) => match reason {
                HaltReason::CreateContractSizeLimit => {
                    TransactionFailureReason::CreateContractSizeLimit
                }
                HaltReason::OpcodeNotFound | HaltReason::InvalidFEOpcode => {
                    TransactionFailureReason::OpcodeNotFound
                }
                HaltReason::OutOfGas(error) => TransactionFailureReason::OutOfGas(error),
                remainder => TransactionFailureReason::Inner(OptimismHaltReason::Base(remainder)),
            },
            remainder => TransactionFailureReason::Inner(remainder),
        }
    }
}