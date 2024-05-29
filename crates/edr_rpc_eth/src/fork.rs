/// Metadata about a forked chain.
#[derive(Clone, Debug)]
pub struct ForkMetadata {
    /// Chain id as returned by `eth_chainId`
    pub chain_id: u64,
    /// Network id as returned by `net_version`
    pub network_id: u64,
    /// The latest block number as returned by `eth_blockNumber`
    pub latest_block_number: u64,
}
