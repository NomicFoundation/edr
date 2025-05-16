use edr_eth::{
    filter::LogFilterOptions, reward_percentile::RewardPercentile, Address, BlockSpec,
    PreEip1898BlockSpec, B256, U256,
};

/// Methods for requests to a remote Ethereum node. Only contains methods
/// supported by the [`edr_rpc_client::RpcClient`].
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "method", content = "params")]
pub enum RequestMethod {
    /// `eth_blockNumber`
    #[serde(rename = "eth_blockNumber", with = "edr_eth::serde::empty_params")]
    BlockNumber(()),
    /// `eth_feeHistory`
    #[serde(rename = "eth_feeHistory")]
    FeeHistory(
        /// block count
        U256,
        /// newest block
        BlockSpec,
        /// reward percentiles
        #[serde(skip_serializing_if = "Option::is_none")]
        Option<Vec<RewardPercentile>>,
    ),
    /// `eth_chainId`
    #[serde(rename = "eth_chainId", with = "edr_eth::serde::empty_params")]
    ChainId(()),
    /// `eth_getBalance`
    #[serde(rename = "eth_getBalance")]
    GetBalance(
        Address,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
    ),
    /// `eth_getBlockByNumber`
    #[serde(rename = "eth_getBlockByNumber")]
    GetBlockByNumber(
        PreEip1898BlockSpec,
        /// include transaction data
        bool,
    ),
    /// `eth_getBlockByHash`
    #[serde(rename = "eth_getBlockByHash")]
    GetBlockByHash(
        /// hash
        B256,
        /// include transaction data
        bool,
    ),
    /// `eth_getCode`
    #[serde(rename = "eth_getCode")]
    GetCode(
        Address,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
    ),
    /// `eth_getLogs`
    #[serde(rename = "eth_getLogs", with = "edr_eth::serde::sequence")]
    GetLogs(LogFilterOptions),
    /// `eth_getStorageAt`
    #[serde(rename = "eth_getStorageAt")]
    GetStorageAt(
        Address,
        /// position
        U256,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
    ),
    /// `eth_getTransactionByHash`
    #[serde(rename = "eth_getTransactionByHash", with = "edr_eth::serde::sequence")]
    GetTransactionByHash(B256),
    /// `eth_getTransactionCount`
    #[serde(rename = "eth_getTransactionCount")]
    GetTransactionCount(
        Address,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
    ),
    /// `eth_getTransactionReceipt`
    #[serde(
        rename = "eth_getTransactionReceipt",
        with = "edr_eth::serde::sequence"
    )]
    GetTransactionReceipt(B256),
    /// `net_version`
    #[serde(rename = "net_version", with = "edr_eth::serde::empty_params")]
    NetVersion(()),
}
