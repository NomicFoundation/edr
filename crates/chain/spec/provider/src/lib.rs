use core::fmt::Debug;
use std::sync::Arc;

use edr_block_api::{EthBlockData, FetchBlockReceipts};
use edr_block_header::{BlockConfig, BlockHeader};
use edr_chain_config::ChainConfig;
use edr_chain_spec::TransactionValidation;
use edr_chain_spec_block::{BlockChainSpec, SyncBlockChainSpec};
use edr_chain_spec_receipt::ReceiptChainSpec;
use edr_chain_spec_rpc::{RpcChainSpec, RpcEthBlock, RpcTransaction, RpcTypeFrom};
use edr_eip1559::BaseFeeParams;
use edr_eip7892::ScheduledBlobParams;
use edr_primitives::{HashMap, B256};
use edr_transaction::{TransactionAndBlock, TransactionType};

/// Trait for specifying the types needed to implement a chain-specific JSON-RPC
/// provider.
pub trait ProviderChainSpec: BlockChainSpec<
        Block: 'static,
        Hardfork: 'static + Debug + PartialOrd,
        LocalBlock: 'static
                        + FetchBlockReceipts<Arc<<Self as ReceiptChainSpec>::Receipt>, Error: Debug>,
        Receipt: 'static + TryFrom<<Self as RpcChainSpec>::RpcReceipt, Error: Send + Sync>,
        RpcBlock<B256>: RpcEthBlock,
        RpcReceipt: RpcTypeFrom<Self::Receipt, Hardfork = Self::Hardfork>,
        RpcTransaction: RpcTransaction
                            + RpcTypeFrom<
            TransactionAndBlock<Arc<Self::Block>, Self::SignedTransaction>,
            Hardfork = Self::Hardfork,
        >,
        SignedTransaction: 'static
                               + Clone
                               + Debug
                               + TransactionType
                               + TransactionValidation<ValidationError: PartialEq>,
    > + BlockChainSpec<
        RpcBlock<<Self as RpcChainSpec>::RpcTransaction>: RpcEthBlock
                                                              + TryInto<
            EthBlockData<Self::SignedTransaction>,
            Error: Send + Sync + std::error::Error,
        >,
    >
{
    /// The minimum difficulty for the Ethash proof-of-work algorithm.
    const MIN_ETHASH_DIFFICULTY: u64;

    /// Returns the chain configurations for this chain type.
    fn chain_configs() -> &'static HashMap<u64, ChainConfig<Self::Hardfork>>;

    /// Returns the default base fee params to fallback to for the given spec
    fn default_base_fee_params() -> &'static BaseFeeParams<Self::Hardfork>;

    /// Returns the default scheduled blob params to fallback to for the given
    /// spec
    fn default_schedulded_blob_params() -> Option<ScheduledBlobParams>;

    /// Returns the `base_fee_per_gas` for the next block.
    fn next_base_fee_per_gas(
        header: &BlockHeader,
        hardfork: Self::Hardfork,
        default_base_fee_params: &BaseFeeParams<Self::Hardfork>,
    ) -> u128;
}

/// Trait for [`ProviderChainSpec`] that meets all requirements for synchronous
/// operations.
pub trait SyncProviderChainSpec: ProviderChainSpec + SyncBlockChainSpec {}

impl<ChainSpecT: ProviderChainSpec + SyncBlockChainSpec> SyncProviderChainSpec for ChainSpecT {}

/// Returns the default block configuration for the given chain specification.
pub fn default_block_config<ChainSpecT: ProviderChainSpec>(
    hardfork: ChainSpecT::Hardfork,
) -> BlockConfig<ChainSpecT::Hardfork> {
    BlockConfig {
        base_fee_params: ChainSpecT::default_base_fee_params().clone(),
        hardfork,
        min_ethash_difficulty: ChainSpecT::MIN_ETHASH_DIFFICULTY,
        scheduled_blob_params: ChainSpecT::default_schedulded_blob_params(),
    }
}
