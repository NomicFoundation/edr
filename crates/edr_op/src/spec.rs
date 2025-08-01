use core::fmt::Debug;
use std::sync::Arc;

use alloy_rlp::RlpEncodable;
use edr_eth::{
    block::{BlobGas, Header, PartialHeader},
    eips::{
        eip1559::{BaseFeeParams, ConstantBaseFeeParams, ForkBaseFeeParams},
        eip4844,
    },
    l1::{self, BlockEnv},
    spec::{ChainHardfork, ChainSpec, EthHeaderConstants},
};
use edr_evm::{
    evm::Evm,
    interpreter::{EthInstructions, EthInterpreter, InterpreterResult},
    precompile::PrecompileProvider,
    spec::{BlockEnvConstructor, ContextForChainSpec, GenesisBlockFactory, RuntimeSpec},
    state::Database,
    transaction::{TransactionError, TransactionErrorForChainSpec, TransactionValidation},
    BlockReceipts, EthLocalBlockForChainSpec, LocalCreationError, RemoteBlock,
    RemoteBlockConversionError, SyncBlock,
};
use edr_napi_core::{
    napi,
    spec::{marshal_response_data, Response, SyncNapiSpec},
};
use edr_provider::{time::TimeSinceEpoch, ProviderSpec, TransactionFailureReason};
use edr_rpc_eth::{jsonrpc, spec::RpcSpec};
use edr_solidity::contract_decoder::ContractDecoder;
use op_revm::{precompiles::OpPrecompiles, L1BlockInfo, OpEvm};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    block::{self, LocalBlock},
    eip1559::encode_dynamic_base_fee_params,
    eip2718::TypedEnvelope,
    hardfork,
    receipt::{self, BlockReceiptFactory},
    rpc,
    transaction::{self, InvalidTransaction},
    OpHaltReason, OpSpecId,
};

/// Chain specification for the Ethereum JSON-RPC API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct OpChainSpec;

impl RpcSpec for OpChainSpec {
    type ExecutionReceipt<Log> = TypedEnvelope<receipt::Execution<Log>>;
    type RpcBlock<Data>
        = edr_rpc_eth::Block<Data>
    where
        Data: Default + DeserializeOwned + Serialize;
    type RpcCallRequest = edr_rpc_eth::CallRequest;
    type RpcReceipt = rpc::BlockReceipt;
    type RpcTransaction = rpc::Transaction;
    type RpcTransactionRequest = edr_rpc_eth::TransactionRequest;
}

impl ChainHardfork for OpChainSpec {
    type Hardfork = OpSpecId;
}

impl ChainSpec for OpChainSpec {
    type BlockEnv = l1::BlockEnv;
    type Context = L1BlockInfo;
    type HaltReason = OpHaltReason;
    type SignedTransaction = transaction::Signed;
}

impl GenesisBlockFactory for OpChainSpec {
    type CreationError = LocalCreationError;

    type LocalBlock = <Self as RuntimeSpec>::LocalBlock;

    fn genesis_block(
        genesis_diff: edr_evm::state::StateDiff,
        hardfork: Self::Hardfork,
        mut options: edr_evm::GenesisBlockOptions,
    ) -> Result<Self::LocalBlock, Self::CreationError> {
        if hardfork >= OpSpecId::HOLOCENE {
            // If no option is provided, fill the `extra_data` field with the dynamic
            // EIP-1559 parameters.
            let extra_data = options.extra_data.unwrap_or_else(|| {
                // TODO: https://github.com/NomicFoundation/edr/issues/887
                // Add support for configuring the dynamic base fee parameters.
                let base_fee_params = *Self::BASE_FEE_PARAMS
                    .at_hardfork(hardfork)
                    .expect("Chain spec must have base fee params for post-London hardforks");

                encode_dynamic_base_fee_params(&base_fee_params)
            });

            options.extra_data = Some(extra_data);
        }

        EthLocalBlockForChainSpec::<Self>::with_genesis_state::<Self>(
            genesis_diff,
            hardfork,
            options,
        )
    }
}

impl RuntimeSpec for OpChainSpec {
    type Block = dyn SyncBlock<
        Arc<Self::BlockReceipt>,
        Self::SignedTransaction,
        Error = <Self::LocalBlock as BlockReceipts<Arc<Self::BlockReceipt>>>::Error,
    >;

    type BlockBuilder<
        'builder,
        BlockchainErrorT: 'builder + Send + std::error::Error,
        StateErrorT: 'builder + Send + std::error::Error,
    > = block::Builder<'builder, BlockchainErrorT, StateErrorT>;

    type BlockReceipt = receipt::Block;
    type BlockReceiptFactory = BlockReceiptFactory;

    type Evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = edr_evm::state::DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: edr_evm::inspector::Inspector<edr_evm::spec::ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    > = OpEvm<
        ContextForChainSpec<Self, DatabaseT>,
        InspectorT,
        EthInstructions<EthInterpreter, ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT,
    >;

    type LocalBlock = LocalBlock;

    type PrecompileProvider<
        BlockchainErrorT,
        DatabaseT: Database<Error = edr_evm::state::DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        StateErrorT,
    > = OpPrecompiles;

    type ReceiptBuilder = receipt::execution::Builder;
    type RpcBlockConversionError = RemoteBlockConversionError<Self::RpcTransactionConversionError>;
    type RpcReceiptConversionError = rpc::receipt::ConversionError;
    type RpcTransactionConversionError = rpc::transaction::ConversionError;

    fn cast_local_block(local_block: Arc<Self::LocalBlock>) -> Arc<Self::Block> {
        local_block
    }

    fn cast_remote_block(remote_block: Arc<RemoteBlock<Self>>) -> Arc<Self::Block> {
        remote_block
    }

    fn cast_transaction_error<BlockchainErrorT, StateErrorT>(
        error: <Self::SignedTransaction as TransactionValidation>::ValidationError,
    ) -> TransactionErrorForChainSpec<BlockchainErrorT, Self, StateErrorT> {
        match error {
            InvalidTransaction::Base(l1::InvalidTransaction::LackOfFundForMaxFee {
                fee,
                balance,
            }) => TransactionError::LackOfFundForMaxFee { fee, balance },
            remainder => TransactionError::InvalidTransaction(remainder),
        }
    }

    fn chain_hardfork_activations(
        chain_id: u64,
    ) -> Option<&'static edr_evm::hardfork::Activations<Self::Hardfork>> {
        hardfork::chain_hardfork_activations(chain_id)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        hardfork::chain_name(chain_id)
    }

    fn evm_with_inspector<
        BlockchainErrorT,
        DatabaseT: Database<Error = edr_evm::state::DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: edr_evm::inspector::Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    >(
        context: ContextForChainSpec<Self, DatabaseT>,
        inspector: InspectorT,
        precompile_provider: PrecompileProviderT,
    ) -> Self::Evm<BlockchainErrorT, DatabaseT, InspectorT, PrecompileProviderT, StateErrorT> {
        OpEvm(Evm {
            ctx: context,
            inspector,
            instruction: EthInstructions::new_mainnet(),
            precompiles: precompile_provider,
        })
    }
}

impl EthHeaderConstants for OpChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<OpSpecId> =
        BaseFeeParams::Variable(ForkBaseFeeParams::new(&[
            (OpSpecId::BEDROCK, ConstantBaseFeeParams::new(50, 6)),
            (OpSpecId::CANYON, ConstantBaseFeeParams::new(250, 6)),
        ]));

    const MIN_ETHASH_DIFFICULTY: u64 = 0;
}

impl SyncNapiSpec for OpChainSpec {
    const CHAIN_TYPE: &'static str = crate::CHAIN_TYPE;

    fn cast_response(
        response: Result<
            edr_provider::ResponseWithTraces<OpHaltReason>,
            edr_provider::ProviderErrorForChainSpec<Self>,
        >,
        _contract_decoder: Arc<ContractDecoder>,
    ) -> napi::Result<edr_napi_core::spec::Response<l1::HaltReason>> {
        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

        marshal_response_data(response).map(|data| Response {
            data,
            // TODO: Add support for Solidity stack traces in OP
            solidity_trace: None,
            traces: Vec::new(),
        })
    }
}

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for OpChainSpec {
    type PooledTransaction = transaction::Pooled;
    type TransactionRequest = transaction::Request;

    fn cast_halt_reason(reason: OpHaltReason) -> TransactionFailureReason<OpHaltReason> {
        match reason {
            OpHaltReason::Base(reason) => match reason {
                l1::HaltReason::CreateContractSizeLimit => {
                    TransactionFailureReason::CreateContractSizeLimit
                }
                l1::HaltReason::OpcodeNotFound | l1::HaltReason::InvalidFEOpcode => {
                    TransactionFailureReason::OpcodeNotFound
                }
                l1::HaltReason::OutOfGas(error) => TransactionFailureReason::OutOfGas(error),
                remainder => TransactionFailureReason::Inner(OpHaltReason::Base(remainder)),
            },
            remainder @ OpHaltReason::FailedDeposit => TransactionFailureReason::Inner(remainder),
        }
    }
}

impl BlockEnvConstructor<PartialHeader> for OpChainSpec {
    fn new_block_env(header: &PartialHeader, hardfork: l1::SpecId) -> Self::BlockEnv {
        BlockEnv {
            number: header.number,
            beneficiary: header.beneficiary,
            timestamp: header.timestamp,
            difficulty: header.difficulty,
            basefee: header.base_fee.map_or(0u64, |base_fee| {
                base_fee.try_into().expect("base fee is too large")
            }),
            gas_limit: header.gas_limit,
            prevrandao: if hardfork >= l1::SpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header.blob_gas.as_ref().map(
                |BlobGas { excess_gas, .. }| {
                    eip4844::BlobExcessGasAndPrice::new(*excess_gas, hardfork >= l1::SpecId::PRAGUE)
                },
            ),
        }
    }
}

impl BlockEnvConstructor<Header> for OpChainSpec {
    fn new_block_env(header: &Header, hardfork: l1::SpecId) -> Self::BlockEnv {
        BlockEnv {
            number: header.number,
            beneficiary: header.beneficiary,
            timestamp: header.timestamp,
            difficulty: header.difficulty,
            basefee: header.base_fee_per_gas.map_or(0u64, |base_fee| {
                base_fee.try_into().expect("base fee is too large")
            }),
            gas_limit: header.gas_limit,
            prevrandao: if hardfork >= l1::SpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header.blob_gas.as_ref().map(
                |BlobGas { excess_gas, .. }| {
                    eip4844::BlobExcessGasAndPrice::new(*excess_gas, hardfork >= l1::SpecId::PRAGUE)
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {

    use edr_eth::{
        block::{BlobGas, Header},
        l1, Address, Bloom, Bytes, B256, B64, U256,
    };
    use edr_evm::spec::BlockEnvConstructor as _;

    use crate::spec::OpChainSpec;

    fn build_block_header(blob_gas: Option<BlobGas>) -> Header {
        Header {
            parent_hash: B256::default(),
            ommers_hash: B256::default(),
            beneficiary: Address::default(),
            state_root: B256::default(),
            transactions_root: B256::default(),
            receipts_root: B256::default(),
            logs_bloom: Bloom::default(),
            difficulty: U256::default(),
            number: 124,
            gas_limit: u64::default(),
            gas_used: 1337,
            timestamp: 0,
            extra_data: Bytes::default(),
            mix_hash: B256::default(),
            nonce: B64::from(99u64),
            base_fee_per_gas: None,
            withdrawals_root: None,
            blob_gas,
            parent_beacon_block_root: None,
            requests_hash: Some(B256::random()),
        }
    }

    #[test]
    fn op_block_constructor_should_not_default_excess_blob_gas_for_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = OpChainSpec::new_block_env(&header, l1::SpecId::CANCUN);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn op_block_constructor_should_not_default_excess_blob_gas_before_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = OpChainSpec::new_block_env(&header, l1::SpecId::SHANGHAI);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn op_block_constructor_should_not_default_excess_blob_gas_after_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = OpChainSpec::new_block_env(&header, l1::SpecId::PRAGUE);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn op_block_constructor_should_use_existing_excess_blob_gas() {
        let excess_gas = 0x80000u64;
        let blob_gas = BlobGas {
            excess_gas,
            gas_used: 0x80000u64,
        };
        let header = build_block_header(Some(blob_gas)); // blob gas present

        let block = OpChainSpec::new_block_env(&header, l1::SpecId::CANCUN);

        let blob_excess_gas = block
            .blob_excess_gas_and_price
            .expect("Blob excess gas should be set");
        assert_eq!(blob_excess_gas.excess_blob_gas, excess_gas);
    }
}
