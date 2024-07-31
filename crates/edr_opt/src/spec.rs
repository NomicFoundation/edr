use alloy_rlp::RlpEncodable;
use edr_eth::{
    block::{BlobGas, PartialHeader},
    chain_spec::EthHeaderConstants,
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams, ForkBaseFeeParams},
    env::{BlobExcessGasAndPrice, BlockEnv},
    U256,
};
use edr_evm::{
    chain_spec::{BlockEnvConstructor, ChainSpec},
    RemoteBlockConversionError,
};
use edr_rpc_eth::spec::RpcSpec;
use revm::{
    handler::register::HandleRegisters,
    optimism::{OptimismHaltReason, OptimismSpecId},
    EvmHandler,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{eip2718::TypedEnvelope, hardfork, receipt, rpc, transaction};

/// Chain specification for the Ethereum JSON-RPC API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct OptimismChainSpec;

impl RpcSpec for OptimismChainSpec {
    type ExecutionReceipt<Log> = TypedEnvelope<receipt::Execution<Log>>;
    type RpcBlock<Data> = edr_rpc_eth::Block<Data> where Data: Default + DeserializeOwned + Serialize;
    type RpcReceipt = rpc::BlockReceipt;
    type RpcTransaction = rpc::Transaction;
}

impl revm::primitives::EvmWiring for OptimismChainSpec {
    type Block = edr_eth::env::BlockEnv;
    type Transaction = transaction::Signed;
    type Hardfork = OptimismSpecId;
    type HaltReason = OptimismHaltReason;
}

impl revm::EvmWiring for OptimismChainSpec {
    type Context = revm::optimism::Context;

    fn handler<'evm, EXT, DB>(hardfork: Self::Hardfork) -> EvmHandler<'evm, Self, EXT, DB>
    where
        DB: revm::Database,
    {
        let mut handler = EvmHandler::mainnet_with_spec(hardfork);

        handler.append_handler_register(HandleRegisters::Plain(
            revm::optimism::optimism_handle_register::<OptimismChainSpec, DB, EXT>,
        ));

        handler
    }
}

impl BlockEnvConstructor<OptimismChainSpec> for revm::primitives::BlockEnv {
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

impl ChainSpec for OptimismChainSpec {
    type ReceiptBuilder = receipt::execution::Builder;
    type RpcBlockConversionError = RemoteBlockConversionError<Self>;
    type RpcReceiptConversionError = rpc::receipt::ConversionError;
    type RpcTransactionConversionError = rpc::transaction::ConversionError;

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
    const BASE_FEE_PARAMS: BaseFeeParams<Self> =
        BaseFeeParams::Variable(ForkBaseFeeParams::new(&[
            (OptimismSpecId::LONDON, ConstantBaseFeeParams::new(50, 6)),
            (OptimismSpecId::CANYON, ConstantBaseFeeParams::new(250, 6)),
        ]));

    const MIN_ETHASH_DIFFICULTY: u64 = 0;
}
