use alloy_dyn_abi::eip712::TypedData;
use derive_where::derive_where;
use edr_eth::{
    filter::{LogFilterOptions, SubscriptionType},
    serde::{optional_single_to_sequence, sequence_to_optional_single},
    Address, BlockSpec, Bytes, PreEip1898BlockSpec, B256, U128, U256, U64,
};
use edr_rpc_eth::{spec::RpcSpec, StateOverrideOptions};
use edr_solidity::artifacts::{CompilerInput, CompilerOutput};
use serde::{Deserialize, Serialize};

use super::serde::{RpcAddress, Timestamp};
use crate::requests::{debug::DebugTraceConfig, hardhat::rpc_types::ResetProviderConfig};

mod optional_block_spec {
    use super::BlockSpec;

    pub fn latest() -> Option<BlockSpec> {
        Some(BlockSpec::latest())
    }

    pub fn pending() -> Option<BlockSpec> {
        Some(BlockSpec::pending())
    }
}

/// for an invoking a method on a remote ethereum node
#[derive(Deserialize, Serialize)]
#[derive_where(Clone, Debug, PartialEq; ChainSpecT::RpcCallRequest, ChainSpecT::RpcTransactionRequest)]
#[serde(bound = "", tag = "method", content = "params")]
pub enum MethodInvocation<ChainSpecT: RpcSpec> {
    /// `eth_accounts`
    #[serde(rename = "eth_accounts", with = "edr_eth::serde::empty_params")]
    Accounts(()),
    /// `eth_blobBaseFee`
    #[serde(rename = "eth_blobBaseFee", with = "edr_eth::serde::empty_params")]
    BlobBaseFee(()),
    /// `eth_blockNumber`
    #[serde(rename = "eth_blockNumber", with = "edr_eth::serde::empty_params")]
    BlockNumber(()),
    /// `eth_call`
    #[serde(rename = "eth_call")]
    Call(
        ChainSpecT::RpcCallRequest,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")] Option<StateOverrideOptions>,
    ),
    /// `eth_chainId`
    #[serde(rename = "eth_chainId", with = "edr_eth::serde::empty_params")]
    ChainId(()),
    /// `eth_coinbase`
    #[serde(rename = "eth_coinbase", with = "edr_eth::serde::empty_params")]
    Coinbase(()),
    /// `eth_estimateGas`
    #[serde(rename = "eth_estimateGas")]
    EstimateGas(
        ChainSpecT::RpcCallRequest,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::pending"
        )]
        Option<BlockSpec>,
    ),
    /// `eth_sign`
    #[serde(rename = "eth_sign")]
    EthSign(
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
        Bytes,
    ),
    /// `eth_feeHistory`
    #[serde(rename = "eth_feeHistory")]
    FeeHistory(
        /// block count
        U256,
        /// newest block
        BlockSpec,
        /// reward percentiles
        #[serde(default, skip_serializing_if = "Option::is_none")]
        Option<Vec<f64>>,
    ),
    /// `eth_gasPrice`
    #[serde(rename = "eth_gasPrice", with = "edr_eth::serde::empty_params")]
    GasPrice(()),
    /// `eth_getBalance`
    #[serde(rename = "eth_getBalance")]
    GetBalance(
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
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
    /// `eth_getBlockTransactionCountByHash`
    #[serde(
        rename = "eth_getBlockTransactionCountByHash",
        with = "edr_eth::serde::sequence"
    )]
    GetBlockTransactionCountByHash(B256),
    /// `eth_getBlockTransactionCountByNumber`
    #[serde(
        rename = "eth_getBlockTransactionCountByNumber",
        with = "edr_eth::serde::sequence"
    )]
    GetBlockTransactionCountByNumber(PreEip1898BlockSpec),
    /// `eth_getCode`
    #[serde(rename = "eth_getCode")]
    GetCode(
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
    ),
    /// `eth_getFilterChanges`
    #[serde(rename = "eth_getFilterChanges", with = "edr_eth::serde::sequence")]
    GetFilterChanges(U256),
    /// `eth_getFilterLogs`
    #[serde(rename = "eth_getFilterLogs", with = "edr_eth::serde::sequence")]
    GetFilterLogs(U256),
    /// `eth_getLogs`
    #[serde(rename = "eth_getLogs", with = "edr_eth::serde::sequence")]
    GetLogs(LogFilterOptions),
    /// `eth_getStorageAt`
    #[serde(rename = "eth_getStorageAt")]
    GetStorageAt(
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
        #[serde(deserialize_with = "crate::requests::serde::deserialize_storage_slot")] U256,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
    ),
    /// `eth_getTransactionByBlockHashAndIndex`
    #[serde(rename = "eth_getTransactionByBlockHashAndIndex")]
    GetTransactionByBlockHashAndIndex(B256, U256),
    /// `eth_getTransactionByBlockNumberAndIndex`
    // Matching Hardhat behavior in not accepting EIP-1898 block tags
    // https://github.com/NomicFoundation/hardhat/blob/06474681f72e1cd895abbec419f6f10be3d8e4ed/packages/hardhat-core/src/internal/hardhat-network/provider/modules/eth.ts#L775
    #[serde(rename = "eth_getTransactionByBlockNumberAndIndex")]
    GetTransactionByBlockNumberAndIndex(PreEip1898BlockSpec, U256),
    /// `eth_getTransactionByHash`
    #[serde(rename = "eth_getTransactionByHash", with = "edr_eth::serde::sequence")]
    GetTransactionByHash(B256),
    /// `eth_getTransactionCount`
    #[serde(rename = "eth_getTransactionCount")]
    GetTransactionCount(
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
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
    /// `eth_maxPriorityFeePerGas`
    #[serde(
        rename = "eth_maxPriorityFeePerGas",
        with = "edr_eth::serde::empty_params"
    )]
    MaxPriorityFeePerGas(()),
    /// `eth_mining`
    #[serde(rename = "eth_mining", with = "edr_eth::serde::empty_params")]
    Mining(()),
    /// `net_listening`
    #[serde(rename = "net_listening", with = "edr_eth::serde::empty_params")]
    NetListening(()),
    /// `net_peerCount`
    #[serde(rename = "net_peerCount", with = "edr_eth::serde::empty_params")]
    NetPeerCount(()),
    /// `net_version`
    #[serde(rename = "net_version", with = "edr_eth::serde::empty_params")]
    NetVersion(()),
    /// `eth_newBlockFilter`
    #[serde(rename = "eth_newBlockFilter", with = "edr_eth::serde::empty_params")]
    NewBlockFilter(()),
    /// `eth_newFilter`
    #[serde(rename = "eth_newFilter", with = "edr_eth::serde::sequence")]
    NewFilter(LogFilterOptions),
    /// `eth_newPendingTransactionFilter`
    #[serde(
        rename = "eth_newPendingTransactionFilter",
        with = "edr_eth::serde::empty_params"
    )]
    NewPendingTransactionFilter(()),
    /// `eth_pendingTransactions`
    #[serde(
        rename = "eth_pendingTransactions",
        with = "edr_eth::serde::empty_params"
    )]
    PendingTransactions(()),
    /// `eth_sendRawTransaction`
    #[serde(rename = "eth_sendRawTransaction", with = "edr_eth::serde::sequence")]
    SendRawTransaction(Bytes),
    /// `eth_sendTransaction`
    #[serde(rename = "eth_sendTransaction", with = "edr_eth::serde::sequence")]
    SendTransaction(ChainSpecT::RpcTransactionRequest),
    /// `personal_sign`
    #[serde(rename = "personal_sign")]
    PersonalSign(
        Bytes,
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
    ),
    /// `eth_signTypedData_v4`
    #[serde(rename = "eth_signTypedData_v4")]
    SignTypedDataV4(
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
        #[serde(deserialize_with = "crate::requests::serde::deserialize_typed_data")] TypedData,
    ),
    /// `eth_subscribe`
    #[serde(rename = "eth_subscribe")]
    Subscribe(
        SubscriptionType,
        #[serde(default, skip_serializing_if = "Option::is_none")] Option<LogFilterOptions>,
    ),
    /// `eth_syncing`
    #[serde(rename = "eth_syncing", with = "edr_eth::serde::empty_params")]
    Syncing(()),
    /// `eth_uninstallFilter`
    #[serde(rename = "eth_uninstallFilter", with = "edr_eth::serde::sequence")]
    UninstallFilter(U256),
    /// `eth_unsubscribe`
    #[serde(rename = "eth_unsubscribe", with = "edr_eth::serde::sequence")]
    Unsubscribe(U256),
    /// `web3_clientVersion`
    #[serde(rename = "web3_clientVersion", with = "edr_eth::serde::empty_params")]
    Web3ClientVersion(()),
    /// `web3_sha3`
    #[serde(rename = "web3_sha3", with = "edr_eth::serde::sequence")]
    Web3Sha3(Bytes),
    /// `evm_increaseTime`
    #[serde(rename = "evm_increaseTime", with = "edr_eth::serde::sequence")]
    EvmIncreaseTime(Timestamp),
    /// `evm_mine`
    #[serde(
        rename = "evm_mine",
        serialize_with = "optional_single_to_sequence",
        deserialize_with = "sequence_to_optional_single"
    )]
    EvmMine(Option<Timestamp>),
    /// `evm_revert`
    #[serde(rename = "evm_revert", with = "edr_eth::serde::sequence")]
    EvmRevert(U64),
    /// `evm_setAutomine`
    #[serde(rename = "evm_setAutomine", with = "edr_eth::serde::sequence")]
    EvmSetAutomine(bool),
    /// `evm_setBlockGasLimit`
    #[serde(rename = "evm_setBlockGasLimit", with = "edr_eth::serde::sequence")]
    EvmSetBlockGasLimit(U64),
    /// `evm_setIntervalMining`
    #[serde(rename = "evm_setIntervalMining", with = "edr_eth::serde::sequence")]
    EvmSetIntervalMining(IntervalConfig),
    /// `evm_setNextBlockTimestamp`
    #[serde(
        rename = "evm_setNextBlockTimestamp",
        with = "edr_eth::serde::sequence"
    )]
    EvmSetNextBlockTimestamp(Timestamp),
    /// `evm_snapshot`
    #[serde(rename = "evm_snapshot", with = "edr_eth::serde::empty_params")]
    EvmSnapshot(()),

    // `debug_traceTransaction`
    #[serde(rename = "debug_traceCall")]
    DebugTraceCall(
        ChainSpecT::RpcCallRequest,
        #[serde(default)] Option<BlockSpec>,
        #[serde(default)] Option<DebugTraceConfig>,
    ),
    // `debug_traceTransaction`
    #[serde(rename = "debug_traceTransaction")]
    DebugTraceTransaction(B256, #[serde(default)] Option<DebugTraceConfig>),

    /// `hardhat_addCompilationResult`
    #[serde(rename = "hardhat_addCompilationResult")]
    AddCompilationResult(
        /// solc version:
        String,
        Box<CompilerInput>,
        CompilerOutput,
    ),
    /// `hardhat_dropTransaction`
    #[serde(rename = "hardhat_dropTransaction", with = "edr_eth::serde::sequence")]
    DropTransaction(B256),
    /// `hardhat_getAutomine`
    #[serde(rename = "hardhat_getAutomine", with = "edr_eth::serde::empty_params")]
    GetAutomine(()),
    /// `hardhat_impersonateAccount`
    #[serde(
        rename = "hardhat_impersonateAccount",
        with = "edr_eth::serde::sequence"
    )]
    ImpersonateAccount(RpcAddress),
    /// `hardhat_intervalMine`
    #[serde(rename = "hardhat_intervalMine", with = "edr_eth::serde::empty_params")]
    IntervalMine(()),
    /// `hardhat_metadata`
    #[serde(rename = "hardhat_metadata", with = "edr_eth::serde::empty_params")]
    Metadata(()),
    /// `hardhat_mine`
    #[serde(rename = "hardhat_mine")]
    Mine(
        /// block count:
        #[serde(default, with = "alloy_serde::quantity::opt")]
        Option<u64>,
        /// interval:
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            with = "alloy_serde::quantity::opt"
        )]
        Option<u64>,
    ),
    /// `hardhat_reset`
    #[serde(
        rename = "hardhat_reset",
        serialize_with = "optional_single_to_sequence",
        deserialize_with = "sequence_to_optional_single"
    )]
    Reset(Option<ResetProviderConfig>),
    /// `hardhat_setBalance`
    #[serde(rename = "hardhat_setBalance")]
    SetBalance(
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
        #[serde(deserialize_with = "crate::requests::serde::deserialize_quantity")] U256,
    ),
    /// `hardhat_setCode`
    #[serde(rename = "hardhat_setCode")]
    SetCode(
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
        #[serde(deserialize_with = "crate::requests::serde::deserialize_data")] Bytes,
    ),
    /// `hardhat_setCoinbase`
    #[serde(rename = "hardhat_setCoinbase", with = "edr_eth::serde::sequence")]
    SetCoinbase(#[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address),
    /// `hardhat_setLoggingEnabled`
    #[serde(
        rename = "hardhat_setLoggingEnabled",
        with = "edr_eth::serde::sequence"
    )]
    SetLoggingEnabled(bool),
    /// `hardhat_setMinGasPrice`
    #[serde(rename = "hardhat_setMinGasPrice", with = "edr_eth::serde::sequence")]
    SetMinGasPrice(U128),
    /// `hardhat_setNextBlockBaseFeePerGas`
    #[serde(
        rename = "hardhat_setNextBlockBaseFeePerGas",
        with = "edr_eth::serde::sequence"
    )]
    SetNextBlockBaseFeePerGas(U128),
    /// `hardhat_setNonce`
    #[serde(rename = "hardhat_setNonce")]
    SetNonce(
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
        #[serde(
            deserialize_with = "crate::requests::serde::deserialize_nonce",
            serialize_with = "alloy_serde::quantity::serialize"
        )]
        u64,
    ),
    /// `hardhat_setPrevRandao`
    #[serde(rename = "hardhat_setPrevRandao", with = "edr_eth::serde::sequence")]
    SetPrevRandao(B256),
    /// `hardhat_setStorageAt`
    #[serde(rename = "hardhat_setStorageAt")]
    SetStorageAt(
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")] Address,
        #[serde(deserialize_with = "crate::requests::serde::deserialize_storage_key")] U256,
        #[serde(with = "crate::requests::serde::storage_value")] U256,
    ),
    /// `hardhat_stopImpersonatingAccount`
    #[serde(
        rename = "hardhat_stopImpersonatingAccount",
        with = "edr_eth::serde::sequence"
    )]
    StopImpersonatingAccount(RpcAddress),
}

impl<ChainSpecT: RpcSpec> MethodInvocation<ChainSpecT> {
    /// Retrieves the instance's method name.
    pub fn method_name(&self) -> &'static str {
        match self {
            MethodInvocation::Accounts(_) => "eth_accounts",
            MethodInvocation::BlobBaseFee(_) => "eth_blobBaseFee",
            MethodInvocation::BlockNumber(_) => "eth_blockNumber",
            MethodInvocation::Call(_, _, _) => "eth_call",
            MethodInvocation::ChainId(_) => "eth_chainId",
            MethodInvocation::Coinbase(_) => "eth_coinbase",
            MethodInvocation::EstimateGas(_, _) => "eth_estimateGas",
            MethodInvocation::EthSign(_, _) => "eth_sign",
            MethodInvocation::FeeHistory(_, _, _) => "eth_feeHistory",
            MethodInvocation::GasPrice(_) => "eth_gasPrice",
            MethodInvocation::GetBalance(_, _) => "eth_getBalance",
            MethodInvocation::GetBlockByNumber(_, _) => "eth_getBlockByNumber",
            MethodInvocation::GetBlockByHash(_, _) => "eth_getBlockByHash",
            MethodInvocation::GetBlockTransactionCountByHash(_) => {
                "eth_getBlockTransactionCountByHash"
            }
            MethodInvocation::GetBlockTransactionCountByNumber(_) => {
                "eth_getBlockTransactionCountByNumber"
            }
            MethodInvocation::GetCode(_, _) => "eth_getCode",
            MethodInvocation::GetFilterChanges(_) => "eth_getFilterChanges",
            MethodInvocation::GetFilterLogs(_) => "eth_getFilterLogs",
            MethodInvocation::GetLogs(_) => "eth_getLogs",
            MethodInvocation::GetStorageAt(_, _, _) => "eth_getStorageAt",
            MethodInvocation::GetTransactionByBlockHashAndIndex(_, _) => {
                "eth_getTransactionByBlockHashAndIndex"
            }
            MethodInvocation::GetTransactionByBlockNumberAndIndex(_, _) => {
                "eth_getTransactionByBlockNumberAndIndex"
            }
            MethodInvocation::GetTransactionByHash(_) => "eth_getTransactionByHash",
            MethodInvocation::GetTransactionCount(_, _) => "eth_getTransactionCount",
            MethodInvocation::GetTransactionReceipt(_) => "eth_getTransactionReceipt",
            MethodInvocation::MaxPriorityFeePerGas(_) => "eth_maxPriorityFeePerGas",
            MethodInvocation::Mining(_) => "eth_mining",
            MethodInvocation::NetListening(_) => "net_listening",
            MethodInvocation::NetPeerCount(_) => "net_peerCount",
            MethodInvocation::NetVersion(_) => "net_version",
            MethodInvocation::NewBlockFilter(_) => "eth_newBlockFilter",
            MethodInvocation::NewFilter(_) => "eth_newFilter",
            MethodInvocation::NewPendingTransactionFilter(_) => "eth_newPendingTransactionFilter",
            MethodInvocation::PendingTransactions(_) => "eth_pendingTransactions",
            MethodInvocation::PersonalSign(_, _) => "personal_sign",
            MethodInvocation::SendRawTransaction(_) => "eth_sendRawTransaction",
            MethodInvocation::SendTransaction(_) => "eth_sendTransaction",
            MethodInvocation::SignTypedDataV4(_, _) => "eth_signTypedData_v4",
            MethodInvocation::Subscribe(_, _) => "eth_subscribe",
            MethodInvocation::Syncing(_) => "eth_syncing",
            MethodInvocation::UninstallFilter(_) => "eth_uninstallFilter",
            MethodInvocation::Unsubscribe(_) => "eth_unsubscribe",
            MethodInvocation::Web3ClientVersion(_) => "web3_clientVersion",
            MethodInvocation::Web3Sha3(_) => "web3_sha3",
            MethodInvocation::EvmIncreaseTime(_) => "evm_increaseTime",
            MethodInvocation::EvmMine(_) => "evm_mine",
            MethodInvocation::EvmRevert(_) => "evm_revert",
            MethodInvocation::EvmSetAutomine(_) => "evm_setAutomine",
            MethodInvocation::EvmSetBlockGasLimit(_) => "evm_setBlockGasLimit",
            MethodInvocation::EvmSetIntervalMining(_) => "evm_setIntervalMining",
            MethodInvocation::EvmSetNextBlockTimestamp(_) => "evm_setNextBlockTimestamp",
            MethodInvocation::EvmSnapshot(_) => "evm_snapshot",
            MethodInvocation::DebugTraceCall(_, _, _) => "debug_traceCall",
            MethodInvocation::DebugTraceTransaction(_, _) => "debug_traceTransaction",
            MethodInvocation::AddCompilationResult(_, _, _) => "hardhat_addCompilationResult",
            MethodInvocation::DropTransaction(_) => "hardhat_dropTransaction",
            MethodInvocation::GetAutomine(_) => "hardhat_getAutomine",
            MethodInvocation::ImpersonateAccount(_) => "hardhat_impersonateAccount",
            MethodInvocation::IntervalMine(_) => "hardhat_intervalMine",
            MethodInvocation::Metadata(_) => "hardhat_metadata",
            MethodInvocation::Mine(_, _) => "hardhat_mine",
            MethodInvocation::Reset(_) => "hardhat_reset",
            MethodInvocation::SetBalance(_, _) => "hardhat_setBalance",
            MethodInvocation::SetCode(_, _) => "hardhat_setCode",
            MethodInvocation::SetCoinbase(_) => "hardhat_setCoinbase",
            MethodInvocation::SetLoggingEnabled(_) => "hardhat_setLoggingEnabled",
            MethodInvocation::SetMinGasPrice(_) => "hardhat_setMinGasPrice",
            MethodInvocation::SetNextBlockBaseFeePerGas(_) => "hardhat_setNextBlockBaseFeePerGas",
            MethodInvocation::SetNonce(_, _) => "hardhat_setNonce",
            MethodInvocation::SetPrevRandao(_) => "hardhat_setPrevRandao",
            MethodInvocation::SetStorageAt(_, _, _) => "hardhat_setStorageAt",
            MethodInvocation::StopImpersonatingAccount(_) => "hardhat_stopImpersonatingAccount",
        }
    }
}

/// an input that can be either a single `u64` or an array of two `u64` values
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum IntervalConfig {
    /// A fixed value; or disabled, when zero.
    FixedOrDisabled(u64),
    /// an array of two `u64` values
    Range([u64; 2]),
}
