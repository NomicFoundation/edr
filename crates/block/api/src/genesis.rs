use edr_block_header::{BlobGas, BlockConfig, HeaderOverrides};
use edr_eip1559::BaseFeeParams;
use edr_chain_spec::HardforkChainSpec;
use edr_primitives::{Bytes, B256};
use edr_state_api::StateDiff;

/// Options for creating a genesis block.
#[derive(Default)]
pub struct GenesisBlockOptions<HardforkT> {
    /// The block's extra data
    pub extra_data: Option<Bytes>,
    /// The block's gas limit
    pub gas_limit: Option<u64>,
    /// The block's timestamp
    pub timestamp: Option<u64>,
    /// The block's mix hash (or prevrandao for post-merge blockchains)
    pub mix_hash: Option<B256>,
    /// The block's base gas fee
    pub base_fee: Option<u128>,
    /// Base fee params to calculate `base_fee` if not set
    pub base_fee_params: Option<BaseFeeParams<HardforkT>>,
    /// The block's blob gas (for post-Cancun blockchains)
    pub blob_gas: Option<BlobGas>,
}

impl<HardforkT> From<GenesisBlockOptions<HardforkT>> for HeaderOverrides<HardforkT> {
    fn from(value: GenesisBlockOptions<HardforkT>) -> Self {
        let GenesisBlockOptions {
            extra_data,
            gas_limit,
            timestamp,
            mix_hash,
            base_fee,
            base_fee_params,
            blob_gas,
        } = value;

        Self {
            extra_data,
            gas_limit,
            timestamp,
            mix_hash,
            base_fee,
            base_fee_params,
            blob_gas,
            ..HeaderOverrides::default()
        }
    }
}

/// Trait for constructing a chain-specific genesis block.
pub trait GenesisBlockFactory: HardforkChainSpec {
    /// The error type for genesis block creation.
    type CreationError: std::error::Error;

    /// The local block type.
    type LocalBlock;

    /// Constructs a genesis block for the given chain spec.
    fn genesis_block(
        genesis_diff: StateDiff,
        block_config: BlockConfig<'_, Self::Hardfork>,
        options: GenesisBlockOptions<Self::Hardfork>,
    ) -> Result<Self::LocalBlock, Self::CreationError>;
}

/// A supertrait for [`GenesisBlockFactory`] that is safe to send between
/// threads.
pub trait SyncGenesisBlockFactory:
    GenesisBlockFactory<CreationError: Send + Sync> + Sync + Send
{
}

impl<FactoryT> SyncGenesisBlockFactory for FactoryT where
    FactoryT: GenesisBlockFactory<CreationError: Send + Sync> + Sync + Send
{
}
