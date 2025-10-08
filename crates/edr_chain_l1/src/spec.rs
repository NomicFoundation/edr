use alloy_rlp::RlpEncodable;
use edr_block_api::{GenesisBlockFactory, GenesisBlockOptions};
use edr_block_header::BlockConfig;
use edr_evm_spec::{ChainHardfork, ChainSpec};
use edr_primitives::Bytes;
use edr_rpc_eth::ChainRpcBlock;
use edr_rpc_spec::RpcSpec;
use edr_state_api::StateDiff;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    rpc::{
        block::L1RpcBlock,
        call::L1CallRequest,
        receipt::L1BlockReceipt,
        transaction::{L1RpcTransactionRequest, L1RpcTransactionWithSignature},
    },
    BlockEnv, HaltReason, Hardfork, L1SignedTransaction, TypedEnvelope,
};

/// Ethereum L1 extra data for genesis blocks.
pub const EXTRA_DATA: &[u8] = b"\x12\x34";

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl GenesisBlockFactory for L1ChainSpec {
    type CreationError = LocalCreationError;

    type LocalBlock = EthLocalBlock<
        Self::RpcBlockConversionError,
        Self::BlockReceipt,
        ExecutionReceiptTypeConstructorForChainSpec<Self>,
        Self::Hardfork,
        Self::RpcReceiptConversionError,
        Self::SignedTransaction,
    >;

    fn genesis_block(
        genesis_diff: StateDiff,
        block_config: BlockConfig<'_, Self::Hardfork>,
        mut options: GenesisBlockOptions<Self::Hardfork>,
    ) -> Result<Self::LocalBlock, Self::CreationError> {
        // If no option is provided, use the default extra data for L1 Ethereum.
        options.extra_data = Some(
            options
                .extra_data
                .unwrap_or(Bytes::copy_from_slice(EXTRA_DATA)),
        );

        EthLocalBlockForChainSpec::<Self>::with_genesis_state::<Self>(
            genesis_diff,
            block_config,
            options,
        )
    }
}

impl ChainHardfork for L1ChainSpec {
    type Hardfork = Hardfork;
}

impl ChainSpec for L1ChainSpec {
    type BlockEnv = BlockEnv;
    type Context = ();
    type HaltReason = HaltReason;
    type SignedTransaction = L1SignedTransaction;
}

impl ChainRpcBlock for L1ChainSpec {
    type RpcBlock<DataT>
        = L1RpcBlock<DataT>
    where
        DataT: Default + DeserializeOwned + Serialize;
}

impl RpcSpec for L1ChainSpec {
    type ExecutionReceipt<LogT> = TypedEnvelope<edr_receipt::execution::Eip658<LogT>>;
    type RpcCallRequest = L1CallRequest;
    type RpcReceipt = L1BlockReceipt;
    type RpcTransaction = L1RpcTransactionWithSignature;
    type RpcTransactionRequest = L1RpcTransactionRequest;
}
