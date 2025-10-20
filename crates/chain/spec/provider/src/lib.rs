use edr_block_header::BlockHeader;
use edr_chain_config::ChainConfig;
use edr_chain_spec_block::BlockChainSpec;
use edr_eip1559::BaseFeeParams;

pub trait ProviderChainSpec: BlockChainSpec {
    /// The minimum difficulty for the Ethash proof-of-work algorithm.
    const MIN_ETHASH_DIFFICULTY: u64;

    /// Returns the corresponding configuration for the provided chain ID, if it
    /// is associated with this chain specification.
    fn chain_config(chain_id: u64) -> Option<&'static ChainConfig<Self::Hardfork>>;

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
