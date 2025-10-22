use core::fmt::Debug;
use std::sync::Arc;

use edr_block_api::{BlockReceipts, EthBlockData};
use edr_block_header::BlockHeader;
use edr_block_remote::RemoteBlock;
use edr_chain_config::ChainConfig;
use edr_chain_spec::ChainSpec;
use edr_chain_spec_block::BlockChainSpec;
use edr_eip1559::BaseFeeParams;
use edr_primitives::{HashMap, B256};
use edr_receipt_spec::ReceiptChainSpec;
use edr_rpc_spec::{RpcChainSpec, RpcEthBlock, RpcTransaction};
use edr_utils::CastArcFrom;

pub trait ProviderChainSpec:
    BlockChainSpec<
        Block: CastArcFrom<<Self as BlockChainSpec>::LocalBlock>
                   + CastArcFrom<
            RemoteBlock<
                <Self as ReceiptChainSpec>::Receipt,
                Self,
                <Self as RpcChainSpec>::RpcReceipt,
                <Self as RpcChainSpec>::RpcTransaction,
                <Self as ChainSpec>::SignedTransaction,
            >,
        >,
        Hardfork: PartialOrd,
        LocalBlock: BlockReceipts<Arc<<Self as ReceiptChainSpec>::Receipt>, Error: Debug>,
        Receipt: TryFrom<<Self as RpcChainSpec>::RpcReceipt, Error: Send + Sync>,
        RpcBlock<B256>: RpcEthBlock,
        RpcTransaction: RpcTransaction,
        SignedTransaction: Clone + serde::de::DeserializeOwned + serde::Serialize,
    > + BlockChainSpec<
        RpcBlock<<Self as RpcChainSpec>::RpcTransaction>: RpcEthBlock
                                                              + TryInto<
            EthBlockData<Self::SignedTransaction>,
            Error: Send + Sync + std::error::Error,
        >,
    > + Sized
{
    /// The minimum difficulty for the Ethash proof-of-work algorithm.
    const MIN_ETHASH_DIFFICULTY: u64;

    /// Returns the chain configurations for this chain type.
    fn chain_configs() -> &'static HashMap<u64, ChainConfig<Self::Hardfork>>;

    /// Returns the default base fee params to fallback to for the given spec
    fn default_base_fee_params() -> &'static BaseFeeParams<Self::Hardfork>;

    /// Returns the `base_fee_per_gas` for the next block.
    fn next_base_fee_per_gas(
        header: &BlockHeader,
        chain_id: u64,
        hardfork: Self::Hardfork,
        base_fee_params_overrides: Option<&BaseFeeParams<Self::Hardfork>>,
    ) -> u128;
}
