use alloy_dyn_abi::eip712::TypedData;
use alloy_rpc_types_trace::geth::GethDebugTracingOptions;
use derive_where::derive_where;
use edr_chain_spec_rpc::RpcChainSpec;
use edr_eth::{
    filter::{LogFilterOptions, SubscriptionType},
    serde::{optional_single_to_sequence, sequence_to_optional_single},
    BlockSpec, PreEip1898BlockSpec,
};
use edr_primitives::{Address, Bytes, StorageKey, B256, U128, U256, U64};
use edr_rpc_eth::StateOverrideOptions;
use serde::{Deserialize, Serialize};

use super::serde::{RpcAddress, Timestamp};

mod optional_block_spec {
    use super::BlockSpec;

    pub fn latest() -> Option<BlockSpec> {
        Some(BlockSpec::latest())
    }

    pub fn pending() -> Option<BlockSpec> {
        Some(BlockSpec::pending())
    }
}

/// For invoking a JSON-RPC method on a local Ethereum development node.
#[derive(Deserialize, Serialize)]
#[derive_where(Clone, Debug, PartialEq; ChainSpecT::RpcCallRequest, ChainSpecT::RpcTransactionRequest)]
#[serde(bound = "", tag = "method", content = "params")]
pub enum MethodInvocation<ChainSpecT: RpcChainSpec> {
    /// # `eth_accounts`
    ///
    /// Returns a list of addresses owned by the provider.
    ///
    /// ## Result
    ///
    /// `Array<DATA, 20 bytes>` - List of addresses owned by the provider.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// [
    ///   "0x0000000000000000000000000000000000000001",
    ///   "0x0000000000000000000000000000000000000002"
    /// ]
    /// ```
    #[serde(rename = "eth_accounts", with = "edr_eth::serde::empty_params")]
    Accounts(()),
    /// # `eth_blobBaseFee`
    ///
    /// Returns the expected base fee per blob gas for the next block.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - The base fee per blob gas in wei.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x1"
    /// ```
    #[serde(rename = "eth_blobBaseFee", with = "edr_eth::serde::empty_params")]
    BlobBaseFee(()),
    /// # `eth_blockNumber`
    ///
    /// Returns the number of the most recent block.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - The current block number.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0xa"
    /// ```
    #[serde(rename = "eth_blockNumber", with = "edr_eth::serde::empty_params")]
    BlockNumber(()),
    /// # `eth_call`
    ///
    /// Executes a new message call immediately without creating a transaction
    /// on the blockchain.
    ///
    /// ## Result
    ///
    /// `DATA` - The return value of the executed contract call.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     {
    ///       "to": "0x0000000000000000000000000000000000000001",
    ///       "data": "0x70a08231000000000000000000000000000000000000000000000000000000000000dead"
    ///     },
    ///     "latest"
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x0000000000000000000000000000000000000000000000000000000000000001"
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Supports an optional third parameter for state overrides.
    /// - If no `from` is provided, uses the first owned account—if present—or
    ///   alternatively the zero address as the caller.
    /// - Gas price defaults to `0` for call requests.
    #[serde(rename = "eth_call")]
    Call(
        /// `Object` - The transaction call object.
        ChainSpecT::RpcCallRequest,
        /// `BlockSpec` - Block number, tag, or EIP-1898 block identifier.
        /// Defaults to `"latest"`.
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
        /// `Object` - State override set. Allows overriding balance, nonce,
        /// code, and storage of accounts during the call.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        Option<StateOverrideOptions>,
    ),
    /// # `eth_chainId`
    ///
    /// Returns the chain ID of the current network.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - The current chain ID.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x539"
    /// ```
    #[serde(rename = "eth_chainId", with = "edr_eth::serde::empty_params")]
    ChainId(()),
    /// # `eth_coinbase`
    ///
    /// Returns the address of the coinbase.
    ///
    /// ## Result
    ///
    /// `DATA, 20 bytes` - The coinbase address.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x0000000000000000000000000000000000000001"
    /// ```
    #[serde(rename = "eth_coinbase", with = "edr_eth::serde::empty_params")]
    Coinbase(()),
    /// # `eth_estimateGas`
    ///
    /// Generates and returns an estimate of the gas required to allow the
    /// transaction to complete. The transaction will not be added to the
    /// blockchain.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - The estimated amount of gas needed.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     {
    ///       "from": "0x0000000000000000000000000000000000000001",
    ///       "to": "0x0000000000000000000000000000000000000002",
    ///       "value": "0xde0b6b3a7640000"
    ///     }
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x5208"
    /// ```
    #[serde(rename = "eth_estimateGas")]
    EstimateGas(
        /// `Object` - The transaction call object.
        ChainSpecT::RpcCallRequest,
        /// `BlockSpec` - Block number, tag, or EIP-1898 block identifier.
        /// Defaults to `"pending"`.
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::pending"
        )]
        Option<BlockSpec>,
    ),
    /// # `eth_sign`
    ///
    /// Calculates an [EIP-191] signature for the provided data.
    ///
    /// [EIP-191]: https://eips.ethereum.org/EIPS/eip-191
    ///
    /// ## Result
    ///
    /// `DATA, 65 bytes` - The signature bytes.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000001",
    ///     "0x48656c6c6f"
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0xa3f207...ee01b"
    /// ```
    #[serde(rename = "eth_sign")]
    EthSign(
        /// `DATA, 20 bytes` - Address of the account to sign with.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `DATA` - Message data to sign.
        Bytes,
    ),
    /// # `eth_feeHistory`
    ///
    /// Returns a collection of historical transaction fee information for the
    /// requested block range, including base fee per gas and effective
    /// priority fee.
    ///
    /// ## Result
    ///
    /// `Object` - Fee history result containing `oldestBlock`,
    /// `baseFeePerGas`, `gasUsedRatio`, and optionally `reward` arrays.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x5", "latest", [25, 75]]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "oldestBlock": "0x1",
    ///   "baseFeePerGas": ["0x3b9aca00", "0x3b9aca00"],
    ///   "gasUsedRatio": [0.5],
    ///   "reward": [["0x3b9aca00", "0x3b9aca00"]]
    /// }
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Only available on London hardfork or later.
    #[serde(rename = "eth_feeHistory")]
    FeeHistory(
        /// `QUANTITY` - Number of blocks in the requested range. Must be
        /// between 1 and 1024.
        U256,
        /// `BlockSpec` - Newest block in the requested range.
        BlockSpec,
        /// `Array<float>` - Monotonically increasing list of percentile
        /// values (0-100) to sample from each block's effective priority
        /// fees.
        Vec<f64>,
    ),
    /// # `eth_gasPrice`
    ///
    /// Returns the current gas price in wei.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - The current gas price in wei.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x3b9aca00"
    /// ```
    #[serde(rename = "eth_gasPrice", with = "edr_eth::serde::empty_params")]
    GasPrice(()),
    /// # `eth_getBalance`
    ///
    /// Returns the balance of the account at the provided address.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - The balance of the account in wei.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000001", "latest"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x0234c8a3397aab58"
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Post-merge block tags (`"safe"`, `"finalized"`) are only available for
    ///   the merge hardfork and later.
    #[serde(rename = "eth_getBalance")]
    GetBalance(
        /// `DATA, 20 bytes` - Address to check the balance of.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `BlockSpec` - Block number, tag, or EIP-1898 block identifier.
        /// Defaults to `"latest"`.
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
    ),
    /// # `eth_getBlockByNumber`
    ///
    /// Returns information about a block by block number.
    ///
    /// ## Result
    ///
    /// `Object|null` - A block object, or `null` when no block was found.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x1", false]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "number": "0x1",
    ///   "hash": "0x000...001",
    ///   "parentHash": "0x000...000",
    ///   "transactions": ["0xabc..."],
    ///   ...
    /// }
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Post-merge block tags (`"safe"`, `"finalized"`) are only available for
    ///   the merge hardfork and later.
    #[serde(rename = "eth_getBlockByNumber")]
    GetBlockByNumber(
        /// `BlockSpec` - Block number or tag (`"latest"`, `"earliest"`,
        /// `"pending"`). Does not accept EIP-1898 format.
        PreEip1898BlockSpec,
        /// `Boolean` - If `true`, returns full transaction objects; if
        /// `false`, returns only transaction hashes.
        bool,
    ),
    /// # `eth_getBlockByHash`
    ///
    /// Returns information about a block by block hash.
    ///
    /// ## Result
    ///
    /// `Object|null` - A block object, or `null` when no block was found.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000000000000000000000000000001",
    ///     true
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "number": "0x1",
    ///   "hash": "0x000...001",
    ///   "transactions": [{ "hash": "0xabc...", "from": "0x..." }, ...],
    ///   ...
    /// }
    /// ```
    #[serde(rename = "eth_getBlockByHash")]
    GetBlockByHash(
        /// `DATA, 32 bytes` - Hash of a block.
        B256,
        /// `Boolean` - If `true`, returns full transaction objects; if
        /// `false`, returns only transaction hashes.
        bool,
    ),
    /// # `eth_getBlockTransactionCountByHash`
    ///
    /// Returns the number of transactions in the block identified by the
    /// provided block hash.
    ///
    /// ## Result
    ///
    /// `QUANTITY|null` - Number of transactions in the block, or `null` if
    /// the block was not found.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000000000000000000000000000001"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x5"
    /// ```
    #[serde(
        rename = "eth_getBlockTransactionCountByHash",
        with = "edr_eth::serde::sequence"
    )]
    GetBlockTransactionCountByHash(
        /// `DATA, 32 bytes` - Hash of a block.
        B256,
    ),
    /// # `eth_getBlockTransactionCountByNumber`
    ///
    /// Returns the number of transactions in the block identified by the
    /// provided block number.
    ///
    /// ## Result
    ///
    /// `QUANTITY|null` - Number of transactions in the block, or `null` if
    /// the block was not found.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x1"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x5"
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Post-merge block tags (`"safe"`, `"finalized"`) are only available for
    ///   the merge hardfork and later.
    #[serde(
        rename = "eth_getBlockTransactionCountByNumber",
        with = "edr_eth::serde::sequence"
    )]
    GetBlockTransactionCountByNumber(
        /// `BlockSpec` - Block number or tag. Does not accept EIP-1898
        /// format.
        PreEip1898BlockSpec,
    ),
    /// # `eth_getCode`
    ///
    /// Returns the bytecode stored at the provided address.
    ///
    /// ## Result
    ///
    /// `DATA` - The bytecode at the provided address.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000001", "latest"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x6080604052..."
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Post-merge block tags (`"safe"`, `"finalized"`) are only available for
    ///   the merge hardfork and later.
    #[serde(rename = "eth_getCode")]
    GetCode(
        /// `DATA, 20 bytes` - Address to retrieve the code from.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `BlockSpec` - Block number, tag, or EIP-1898 block identifier.
        /// Defaults to `"latest"`.
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
    ),
    /// # `eth_getFilterChanges`
    ///
    /// Polling method for the filter with the provided ID. Returns an array of
    /// logs, block hashes, or transaction hashes that occurred since the
    /// last poll, depending on the filter type.
    ///
    /// ## Result
    ///
    /// `Array` - Array of log objects, block hashes, or transaction hashes
    /// depending on the filter type. Returns an empty array if no changes
    /// occurred.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x1"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// []
    /// ```
    #[serde(rename = "eth_getFilterChanges", with = "edr_eth::serde::sequence")]
    GetFilterChanges(
        /// `QUANTITY` - The filter ID returned by `eth_newFilter`,
        /// `eth_newBlockFilter`, or `eth_newPendingTransactionFilter`.
        U256,
    ),
    /// # `eth_getFilterLogs`
    ///
    /// Returns an array of all logs matching the filter with the provided ID.
    /// Unlike `eth_getFilterChanges`, returns all matching logs, not just
    /// changes since the last poll.
    ///
    /// ## Result
    ///
    /// `Array` - Array of log objects, or an empty array if no logs match.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x1"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// []
    /// ```
    #[serde(rename = "eth_getFilterLogs", with = "edr_eth::serde::sequence")]
    GetFilterLogs(
        /// `QUANTITY` - The filter ID returned by `eth_newFilter`.
        U256,
    ),
    /// # `eth_getLogs`
    ///
    /// Returns an array of all logs matching the filter with the provided ID.
    ///
    /// ## Result
    ///
    /// `Array` - Array of log objects, or an empty array if no logs match.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     {
    ///       "fromBlock": "0x1",
    ///       "toBlock": "latest",
    ///       "address": "0x0000000000000000000000000000000000000001"
    ///     }
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// [
    ///   {
    ///     "address": "0x0000000000000000000000000000000000000001",
    ///     "topics": ["0x000..."],
    ///     "data": "0x",
    ///     "blockNumber": "0x1",
    ///     "transactionHash": "0x000...",
    ///     "logIndex": "0x0"
    ///   }
    /// ]
    /// ```
    #[serde(rename = "eth_getLogs", with = "edr_eth::serde::sequence")]
    GetLogs(
        /// `Object` - The filter options: `fromBlock`, `toBlock`,
        /// `address`, `topics`, and `blockHash`. `blockHash` is mutually
        /// exclusive with `fromBlock`/`toBlock`.
        LogFilterOptions,
    ),
    /// # `eth_getProof`
    ///
    /// Returns the Merkle proof for the account corresponding to the provided
    /// address and (optionally) some storage keys.
    ///
    /// ## Result
    ///
    /// `Object` - An account object containing the account's balance, nonce,
    /// code hash, storage hash, account proof, and storage proofs for the
    /// requested storage keys.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000001",
    ///     ["0x0000000000000000000000000000000000000000000000000000000000000000"],
    ///     "latest"
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "address": "0x0000000000000000000000000000000000000001",
    ///   "balance": "0x0",
    ///   "codeHash": "0x...",
    ///   "nonce": "0x0",
    ///   "storageHash": "0x...",
    ///   "accountProof": ["0x..."],
    ///   "storageProof": [
    ///     {
    ///       "key": "0x0000000000000000000000000000000000000000000000000000000000000000",
    ///       "value": "0x0",
    ///       "proof": ["0x..."]
    ///     }
    ///   ]
    /// }
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Post-merge block tags (`"safe"`, `"finalized"`) are only available for
    ///   the merge hardfork and later.
    #[serde(rename = "eth_getProof")]
    GetProof(
        /// `DATA, 20 bytes` - Address of the account.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `Array<DATA, 32 bytes>` - Array of storage keys to generate proofs
        /// for.
        Vec<StorageKey>,
        /// `BlockSpec` - Block number, tag, or EIP-1898 block identifier.
        BlockSpec,
    ),

    /// # `eth_getStorageAt`
    ///
    /// Returns the value from a storage position at a given address.
    ///
    /// ## Result
    ///
    /// `DATA, 32 bytes` - The value at the provided storage position,
    /// zero-padded to 32 bytes.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000001",
    ///     "0x0",
    ///     "latest"
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x0000000000000000000000000000000000000000000000000000000000000000"
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Post-merge block tags (`"safe"`, `"finalized"`) are only available for
    ///   the merge hardfork and later.
    #[serde(rename = "eth_getStorageAt")]
    GetStorageAt(
        /// `DATA, 20 bytes` - Address of the account.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `QUANTITY` - The storage slot index.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_storage_slot")]
        U256,
        /// `BlockSpec` - Block number, tag, or EIP-1898 block identifier.
        /// Defaults to `"latest"`.
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
    ),
    /// # `eth_getTransactionByBlockHashAndIndex`
    ///
    /// Returns information about a transaction by block hash and transaction
    /// index position.
    ///
    /// ## Result
    ///
    /// `Object|null` - A transaction object, or `null` when no transaction
    /// was found.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000000000000000000000000000001",
    ///     "0x0"
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "hash": "0x000...",
    ///   "from": "0x000...",
    ///   "to": "0x000...",
    ///   "blockHash": "0x000...",
    ///   "blockNumber": "0x1",
    ///   "transactionIndex": "0x0"
    /// }
    /// ```
    #[serde(rename = "eth_getTransactionByBlockHashAndIndex")]
    GetTransactionByBlockHashAndIndex(
        /// `DATA, 32 bytes` - Hash of a block.
        B256,
        /// `QUANTITY` - The transaction index position.
        U256,
    ),
    /// # `eth_getTransactionByBlockNumberAndIndex`
    ///
    /// Returns information about a transaction by block number and
    /// transaction index position.
    ///
    /// ## Result
    ///
    /// `Object|null` - A transaction object, or `null` when no transaction
    /// was found.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x1", "0x0"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "hash": "0x000...",
    ///   "from": "0x000...",
    ///   "to": "0x000...",
    ///   "blockNumber": "0x1",
    ///   "transactionIndex": "0x0"
    /// }
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Post-merge block tags (`"safe"`, `"finalized"`) are only available for
    ///   the merge hardfork and later.
    #[serde(rename = "eth_getTransactionByBlockNumberAndIndex")]
    GetTransactionByBlockNumberAndIndex(
        /// `BlockSpec` - Block number or tag. Does not accept EIP-1898
        /// format.
        PreEip1898BlockSpec,
        /// `QUANTITY` - The transaction index position.
        U256,
    ),
    /// # `eth_getTransactionByHash`
    ///
    /// Returns information about a transaction by transaction hash.
    ///
    /// ## Result
    ///
    /// `Object|null` - A transaction object, or `null` when no transaction
    /// was found.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000000000000000000000000000001"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "hash": "0x000...",
    ///   "from": "0x000...",
    ///   "to": "0x000...",
    ///   "blockHash": "0x000...",
    ///   "blockNumber": "0x1"
    /// }
    /// ```
    #[serde(rename = "eth_getTransactionByHash", with = "edr_eth::serde::sequence")]
    GetTransactionByHash(
        /// `DATA, 32 bytes` - The transaction hash.
        B256,
    ),
    /// # `eth_getTransactionCount`
    ///
    /// Returns the nonce of the account corresponding to the provided address.
    ///
    /// NOTE: This method is named `eth_getTransactionCount` for historical
    /// reasons, as up until the pectra hardfork, the nonce was equivalent to
    /// the number of transactions sent from the address. This changed due to
    /// the inclusion of EIP-7702.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - The nonce of the account at the provided address.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000001", "latest"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x1"
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Post-merge block tags (`"safe"`, `"finalized"`) are only available for
    ///   the merge hardfork and later.
    #[serde(rename = "eth_getTransactionCount")]
    GetTransactionCount(
        /// `DATA, 20 bytes` - Address to check the transaction count for.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `BlockSpec` - Block number, tag, or EIP-1898 block identifier.
        /// Defaults to `"latest"`.
        #[serde(
            skip_serializing_if = "Option::is_none",
            default = "optional_block_spec::latest"
        )]
        Option<BlockSpec>,
    ),
    /// # `eth_getTransactionReceipt`
    ///
    /// Returns the receipt of a transaction by transaction hash.
    ///
    /// ## Result
    ///
    /// `Object|null` - A transaction receipt object, or `null` when the
    /// transaction has not been mined.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000000000000000000000000000001"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "transactionHash": "0x000...",
    ///   "blockHash": "0x000...",
    ///   "blockNumber": "0x1",
    ///   "status": "0x1",
    ///   "gasUsed": "0x5208"
    /// }
    /// ```
    #[serde(
        rename = "eth_getTransactionReceipt",
        with = "edr_eth::serde::sequence"
    )]
    GetTransactionReceipt(
        /// `DATA, 32 bytes` - The transaction hash.
        B256,
    ),
    /// # `eth_maxPriorityFeePerGas`
    ///
    /// Returns the current maximum priority fee per gas in wei for
    /// post-EIP-1559 transactions.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - The suggested priority fee per gas in wei.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x3b9aca00"
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Hardcoded to always returns 1 gwei (1,000,000,000 wei).
    #[serde(
        rename = "eth_maxPriorityFeePerGas",
        with = "edr_eth::serde::empty_params"
    )]
    MaxPriorityFeePerGas(()),
    /// # `net_version`
    ///
    /// Returns the current network ID.
    ///
    /// ## Result
    ///
    /// `String` - The current network ID as a decimal string.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "1337"
    /// ```
    #[serde(rename = "net_version", with = "edr_eth::serde::empty_params")]
    NetVersion(()),
    /// # `eth_newBlockFilter`
    ///
    /// Creates a filter that keeps track of new blocks. The filter can be
    /// polled for newly arrived blocks using `eth_getFilterChanges`.
    ///
    /// Filters time out after a period of inactivity.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - A filter ID.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x1"
    /// ```
    #[serde(rename = "eth_newBlockFilter", with = "edr_eth::serde::empty_params")]
    NewBlockFilter(()),
    /// # `eth_newFilter`
    ///
    /// Creates a filter that keeps track of new logs. The filter can be polled
    /// for newly arrived logs using `eth_getFilterLogs` or
    /// `eth_getFilterChanges`.
    ///
    /// Filters time out after a period of inactivity.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - A filter ID.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     {
    ///       "fromBlock": "0x1",
    ///       "toBlock": "latest",
    ///       "address": "0x0000000000000000000000000000000000000001"
    ///     }
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x1"
    /// ```
    #[serde(rename = "eth_newFilter", with = "edr_eth::serde::sequence")]
    NewFilter(
        /// `Object` - The filter options: `fromBlock`, `toBlock`,
        /// `address`, `topics`, and `blockHash`.
        LogFilterOptions,
    ),
    /// # `eth_newPendingTransactionFilter`
    ///
    /// Creates a filter that keeps track of new pending transactions. The
    /// filter can be polled for newly arrived pending transactions using
    /// `eth_getFilterChanges`.
    ///
    /// Filters time out after a period of inactivity.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - A filter ID.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x1"
    /// ```
    #[serde(
        rename = "eth_newPendingTransactionFilter",
        with = "edr_eth::serde::empty_params"
    )]
    NewPendingTransactionFilter(()),
    /// # `eth_pendingTransactions`
    ///
    /// Returns all pending transactions in the mempool.
    ///
    /// ## Result
    ///
    /// `Array<Object>` - List of pending transaction objects.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// []
    /// ```
    #[serde(
        rename = "eth_pendingTransactions",
        with = "edr_eth::serde::empty_params"
    )]
    PendingTransactions(()),
    /// # `eth_sendRawTransaction`
    ///
    /// Submits a pre-signed, RLP-encoded transaction.
    ///
    /// ## Result
    ///
    /// `DATA, 32 bytes` - The transaction hash, or the zero hash if the
    /// transaction is not yet available.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0xf86c0a85...025a0..."]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x0000000000000000000000000000000000000000000000000000000000000001"
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - EIP-4844 transactions are only supported if auto-mining is enabled and
    ///   the mempool is empty.
    #[serde(rename = "eth_sendRawTransaction", with = "edr_eth::serde::sequence")]
    SendRawTransaction(
        /// `DATA` - The signed, RLP-encoded transaction data.
        Bytes,
    ),
    /// # `eth_sendTransaction`
    ///
    /// Signs and submits a transaction request.
    ///
    /// The `from` address is used to identify the signing account, which must
    /// either be impersonated by or its private key must be owned by the
    /// provider.
    ///
    /// ## Result
    ///
    /// `DATA, 32 bytes` - The transaction hash.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     {
    ///       "from": "0x0000000000000000000000000000000000000001",
    ///       "to": "0x0000000000000000000000000000000000000002",
    ///       "value": "0xde0b6b3a7640000"
    ///     }
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x0000000000000000000000000000000000000000000000000000000000000001"
    /// ```
    ///
    /// # Implementation details
    ///
    /// - EIP-4844 transactions are not supported. Please use
    ///   `eth_sendRawTransaction` instead.
    #[serde(rename = "eth_sendTransaction", with = "edr_eth::serde::sequence")]
    SendTransaction(
        /// `Object` - The transaction request object.
        ChainSpecT::RpcTransactionRequest,
    ),
    /// # `personal_sign`
    ///
    /// Calculates an [EIP-191] signature for the provided data.
    ///
    /// [EIP-191]: https://eips.ethereum.org/EIPS/eip-191
    ///
    /// ## Result
    ///
    /// `DATA` - The signature bytes.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x48656c6c6f",
    ///     "0x0000000000000000000000000000000000000001"
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0xa3f207...ee01b"
    /// ```
    #[serde(rename = "personal_sign")]
    PersonalSign(
        /// `DATA` - Message data to sign.
        Bytes,
        /// `DATA, 20 bytes` - Address of the account to sign with.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
    ),
    /// # `eth_signTypedData_v4`
    ///
    /// Signs typed structured data according to [EIP-712]. The private key of
    /// the signing account must be owned by the provider.
    ///
    /// [EIP-712]: https://eips.ethereum.org/EIPS/eip-712
    ///
    /// ## Result
    ///
    /// `DATA` - The signature bytes.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000001",
    ///     {
    ///       "types": {
    ///         "EIP712Domain": [{ "name": "name", "type": "string" }],
    ///         "Mail": [{ "name": "contents", "type": "string" }]
    ///       },
    ///       "primaryType": "Mail",
    ///       "domain": { "name": "Example" },
    ///       "message": { "contents": "Hello" }
    ///     }
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0xa3f207...ee01b"
    /// ```
    #[serde(rename = "eth_signTypedData_v4")]
    SignTypedDataV4(
        /// `DATA, 20 bytes` - Address of the account to sign with.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `Object` - The EIP-712 typed data to sign.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_typed_data")]
        TypedData,
    ),
    /// # `eth_subscribe`
    ///
    /// Starts a subscription to a particular event. For each matching event,
    /// a notification with relevant data is sent. Only available on
    /// WebSocket connections.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - A subscription ID used for identifying and
    /// unsubscribing.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["newHeads"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x1"
    /// ```
    #[serde(rename = "eth_subscribe")]
    Subscribe(
        /// `String` - The subscription type: `"newHeads"`, `"logs"`, or
        /// `"newPendingTransactions"`.
        SubscriptionType,
        /// `Object` - Filter options (only for `"logs"` subscriptions).
        /// Required when subscription type is `"logs"`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        Option<LogFilterOptions>,
    ),
    /// # `eth_syncing`
    ///
    /// Returns whether the node is syncing.
    ///
    /// ## Result
    ///
    /// `Boolean` - `false` since the local development node is never
    /// syncing.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// false
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Always returns `false`.
    #[serde(rename = "eth_syncing", with = "edr_eth::serde::empty_params")]
    Syncing(()),
    /// # `eth_uninstallFilter`
    ///
    /// Uninstalls the filter corresponding to the provided ID.
    ///
    /// ## Result
    ///
    /// `Boolean` - `true` if the filter was successfully uninstalled,
    /// `false` if no filter with the provided ID exists.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x1"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(rename = "eth_uninstallFilter", with = "edr_eth::serde::sequence")]
    UninstallFilter(
        /// `QUANTITY` - The filter ID.
        U256,
    ),
    /// # `eth_unsubscribe`
    ///
    /// Cancels the subscription corresponding to the provided ID.
    ///
    /// ## Result
    ///
    /// `Boolean` - `true` if the subscription was successfully cancelled,
    /// `false` if no subscription with the provided ID exists.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x1"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(rename = "eth_unsubscribe", with = "edr_eth::serde::sequence")]
    Unsubscribe(
        /// `QUANTITY` - The subscription ID.
        U256,
    ),
    /// # `web3_clientVersion`
    ///
    /// Returns the current client version.
    ///
    /// ## Result
    ///
    /// `String` - The current client version string.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "edr/0.6.0/revm/19.0.0"
    /// ```
    #[serde(rename = "web3_clientVersion", with = "edr_eth::serde::empty_params")]
    Web3ClientVersion(()),
    /// # `web3_sha3`
    ///
    /// Returns the Keccak-256 hash of the provided data.
    ///
    /// ## Result
    ///
    /// `DATA, 32 bytes` - The Keccak-256 hash of the input data.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x68656c6c6f"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8"
    /// ```
    #[serde(rename = "web3_sha3", with = "edr_eth::serde::sequence")]
    Web3Sha3(
        /// `DATA` - The data to hash.
        Bytes,
    ),
    /// # `evm_increaseTime`
    ///
    /// Increases the offset between block timestamps by the provided amount of
    /// seconds. Returns the resulting total offset, in seconds.
    ///
    /// ## Result
    ///
    /// `String` - The total time offset in seconds, as a decimal string.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [60]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "60"
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Returns a decimal string, not a hex-encoded quantity.
    #[serde(rename = "evm_increaseTime", with = "edr_eth::serde::sequence")]
    EvmIncreaseTime(
        /// `QUANTITY` - The number of seconds to increase the time by.
        Timestamp,
    ),
    /// # `evm_mine`
    ///
    /// Mines a single block, including as many transactions from the
    /// transaction pool as possible.
    ///
    /// ## Result
    ///
    /// `String` - Always returns `"0"`.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0"
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Returns the string `"0"`, not a hex-encoded quantity.
    #[serde(
        rename = "evm_mine",
        serialize_with = "optional_single_to_sequence",
        deserialize_with = "sequence_to_optional_single"
    )]
    EvmMine(
        /// `QUANTITY` - Optional timestamp for the mined block. If not
        /// provided, the block timestamp is determined automatically.
        Option<Timestamp>,
    ),
    /// # `evm_revert`
    ///
    /// Reverts the state of the blockchain to a previous snapshot. Takes a
    /// single parameter, which is the snapshot ID to revert to.
    ///
    /// ## Result
    ///
    /// `Boolean` - `true` if a snapshot was reverted, `false` if the
    /// snapshot ID is invalid.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x1"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(rename = "evm_revert", with = "edr_eth::serde::sequence")]
    EvmRevert(
        /// `QUANTITY` - The snapshot ID to revert to.
        U64,
    ),
    /// # `evm_setAutomine`
    ///
    /// Enables or disables automatic mining of new blocks with each new
    /// transaction submitted to the provider.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [true]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(rename = "evm_setAutomine", with = "edr_eth::serde::sequence")]
    EvmSetAutomine(
        /// `Boolean` - `true` to enable automining, `false` to disable.
        bool,
    ),
    /// # `evm_setBlockGasLimit`
    ///
    /// Sets the block gas limit for future blocks.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x1c9c380"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - The gas limit must be greater than zero.
    #[serde(rename = "evm_setBlockGasLimit", with = "edr_eth::serde::sequence")]
    EvmSetBlockGasLimit(
        /// `QUANTITY` - The new block gas limit. Must be greater than zero.
        U64,
    ),
    /// # `evm_setIntervalMining`
    ///
    /// Enables, disables, or re-configures mining of blocks at a pre-configured
    /// time interval.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request (disable interval mining):**
    ///
    /// ```json
    /// {
    ///  "params": [0]
    /// }
    /// ```
    ///
    /// **Request (fixed interval):**
    ///
    /// ```json
    /// {
    ///   "params": [5000]
    /// }
    /// ```
    ///
    /// **Request (random interval range):**
    ///
    /// ```json
    /// {
    ///   "params": [[3000, 6000]]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(rename = "evm_setIntervalMining", with = "edr_eth::serde::sequence")]
    EvmSetIntervalMining(
        /// `QUANTITY|Array` - The interval in milliseconds, or a
        /// two-element `[min, max]` array for a random range. Pass `0` to
        /// disable.
        IntervalConfig,
    ),
    /// # `evm_setNextBlockTimestamp`
    ///
    /// Sets the timestamp of the next block. The timestamp must be greater
    /// than the current block's timestamp.
    ///
    /// ## Result
    ///
    /// `String` - The new timestamp as a decimal string.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [1700000000]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// "1700000000"
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Returns a decimal string, not a hex-encoded quantity.
    #[serde(
        rename = "evm_setNextBlockTimestamp",
        with = "edr_eth::serde::sequence"
    )]
    EvmSetNextBlockTimestamp(
        /// `QUANTITY` - The timestamp for the next block (in seconds since
        /// epoch).
        Timestamp,
    ),
    /// # `evm_snapshot`
    ///
    /// Creates a snapshot of the current state of the blockchain. Returns a
    /// snapshot ID that can later be used with `evm_revert` to restore
    /// this state.
    ///
    /// ## Result
    ///
    /// `QUANTITY` - The snapshot ID.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// "0x1"
    /// ```
    #[serde(rename = "evm_snapshot", with = "edr_eth::serde::empty_params")]
    EvmSnapshot(()),
    /// # `debug_traceCall`
    ///
    /// Runs an `eth_call` within the context of a given block and returns
    /// detailed trace information about the execution.
    ///
    /// ## Result
    ///
    /// `Object` - A trace result object depending on the used tracer.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     {
    ///       "to": "0x0000000000000000000000000000000000000001",
    ///       "data": "0x70a08231"
    ///     },
    ///     "latest",
    ///     { "disableMemory": true }
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "pass": true,
    ///   "gasUsed": 21000,
    ///   "output": "0x",
    ///   "structLogs": []
    /// }
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - The block parameter defaults to `"latest"` when omitted.
    /// - The `4byteTracer`, `callTracer`, `noopTracer`, and `prestateTracer`
    ///   from [geth-tracers] are supported.
    ///
    /// [geth-tracers]: https://geth.ethereum.org/docs/developers/evm-tracing/built-in-tracers
    // TODO: Add support for `GethDebugTracingCallOptions`
    // <https://geth.ethereum.org/docs/interacting-with-geth/rpc/ns-debug#debugtracecall>
    #[serde(rename = "debug_traceCall")]
    DebugTraceCall(
        /// `Object` - The transaction call object.
        ChainSpecT::RpcCallRequest,
        /// `BlockSpec` - Block number, tag, or EIP-1898 block identifier.
        /// Defaults to `"latest"`.
        #[serde(default)]
        Option<BlockSpec>,
        /// `Object` - Geth debug tracing options. Supports various
        /// configuration options including `disableStorage`, `disableMemory`,
        /// `disableStack`, `disableReturnData`, and `enableReturnData`.
        #[serde(default)]
        Option<GethDebugTracingOptions>,
    ),
    /// # `debug_traceTransaction`
    ///
    /// Returns detailed trace information about a previously mined
    /// transaction.
    ///
    /// ## Result
    ///
    /// `Object` - A trace result object containing `gasUsed`, `pass`,
    /// `output`, and `structLogs`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000000000000000000000000000001",
    ///     { "disableStorage": true }
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "pass": true,
    ///   "gasUsed": 21000,
    ///   "output": "0x",
    ///   "structLogs": []
    /// }
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - The `4byteTracer`, `callTracer`, `noopTracer`, and `prestateTracer`
    ///   from [geth-tracers] are supported.
    ///
    /// [geth-tracers]: https://geth.ethereum.org/docs/developers/evm-tracing/built-in-tracers
    #[serde(rename = "debug_traceTransaction")]
    DebugTraceTransaction(
        /// `DATA, 32 bytes` - The hash of the transaction to trace.
        B256,
        /// `Object` - Geth debug tracing options. Supports various
        /// configuration options including `disableStorage`, `disableMemory`,
        /// `disableStack`, `disableReturnData`, and `enableReturnData`.
        #[serde(default)]
        Option<GethDebugTracingOptions>,
    ),
    /// # `hardhat_dropTransaction`
    ///
    /// Removes a transaction from the mempool. Triggers an error if the
    /// transaction has already been mined.
    ///
    /// ## Result
    ///
    /// `Boolean` - `true` if the transaction was removed, `false` if the
    /// transaction was not in the pool.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000000000000000000000000000001"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(rename = "hardhat_dropTransaction", with = "edr_eth::serde::sequence")]
    DropTransaction(
        /// `DATA, 32 bytes` - The hash of the pending transaction to drop.
        B256,
    ),
    /// # `hardhat_getAutomine`
    ///
    /// Returns whether automatic mining is enabled.
    ///
    /// ## Result
    ///
    /// `Boolean` - `true` if automining is enabled, `false` otherwise.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(rename = "hardhat_getAutomine", with = "edr_eth::serde::empty_params")]
    GetAutomine(()),
    /// # `hardhat_impersonateAccount`
    ///
    /// Enables sending of transactions on behalf of the provided address, even
    /// if the private key is not available. The impersonated account does
    /// not need to have any balance to send transactions.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000001"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(
        rename = "hardhat_impersonateAccount",
        with = "edr_eth::serde::sequence"
    )]
    ImpersonateAccount(
        /// `DATA, 20 bytes` - The address to impersonate.
        RpcAddress,
    ),
    /// # `hardhat_metadata`
    ///
    /// Returns metadata about the provider instance, including client
    /// version, chain ID, instance ID, and latest block information.
    ///
    /// ## Result
    ///
    /// `Object` - Metadata object containing `clientVersion`, `chainId`,
    /// `instanceId`, `latestBlockNumber`, `latestBlockHash`, and optionally
    /// `forkedNetwork`.
    ///
    /// ## Example
    ///
    /// **Response:**
    ///
    /// ```json
    /// {
    ///   "clientVersion": "edr/0.6.0/revm/19.0.0",
    ///   "chainId": 1337,
    ///   "instanceId": "0x000...",
    ///   "latestBlockNumber": 10,
    ///   "latestBlockHash": "0x000..."
    /// }
    /// ```
    #[serde(rename = "hardhat_metadata", with = "edr_eth::serde::empty_params")]
    Metadata(()),
    /// # `hardhat_mine`
    ///
    /// Mines one or more blocks with an optional fixed time interval
    /// between them.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0xa", "0x3c"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(rename = "hardhat_mine")]
    Mine(
        /// `QUANTITY` - Number of blocks to mine. Defaults to `1`.
        #[serde(default, with = "alloy_serde::quantity::opt")]
        Option<u64>,
        /// `QUANTITY` - Interval in seconds between each mined block.
        /// Defaults to `1`.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            with = "alloy_serde::quantity::opt"
        )]
        Option<u64>,
    ),
    /// # `hardhat_setBalance`
    ///
    /// Modifies the balance of an account.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000001",
    ///     "0xde0b6b3a7640000"
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - This changes the state without mining a block, but instead is tracked
    ///   outside of the blockchain.
    #[serde(rename = "hardhat_setBalance")]
    SetBalance(
        /// `DATA, 20 bytes` - The address whose balance to set.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `QUANTITY` - The new balance in wei.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_quantity")]
        U256,
    ),
    /// # `hardhat_setCode`
    ///
    /// Modifies the bytecode stored at an account's address.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000001",
    ///     "0x6080604052..."
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - This changes the state without mining a block, but instead is tracked
    ///   outside of the blockchain.
    #[serde(rename = "hardhat_setCode")]
    SetCode(
        /// `DATA, 20 bytes` - The address where the code should be stored.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `DATA` - The new bytecode.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_data")]
        Bytes,
    ),
    /// # `hardhat_setCoinbase`
    ///
    /// Sets the coinbase address to be used in new blocks.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000001"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(rename = "hardhat_setCoinbase", with = "edr_eth::serde::sequence")]
    SetCoinbase(
        /// `DATA, 20 bytes` - The new coinbase address.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
    ),
    /// # `hardhat_setLoggingEnabled`
    ///
    /// Enables or disables logging of JSON-RPC requests and EVM execution.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [true]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(
        rename = "hardhat_setLoggingEnabled",
        with = "edr_eth::serde::sequence"
    )]
    SetLoggingEnabled(
        /// `Boolean` - `true` to enable logging, `false` to disable it.
        bool,
    ),
    /// # `hardhat_setMinGasPrice`
    ///
    /// Sets the minimum gas price accepted by the miner for transaction
    /// inclusion.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x3b9aca00"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Only works for pre-London hardforks. Calling this after the London
    ///   hardfork will return an error.
    #[serde(rename = "hardhat_setMinGasPrice", with = "edr_eth::serde::sequence")]
    SetMinGasPrice(
        /// `QUANTITY` - The minimum gas price in wei.
        U128,
    ),
    /// # `hardhat_setNextBlockBaseFeePerGas`
    ///
    /// Sets the base fee per gas that will be used when mining the next block.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x3b9aca00"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Only works for post-London hardforks. Calling this on a pre-London
    ///   hardfork will return an error.
    #[serde(
        rename = "hardhat_setNextBlockBaseFeePerGas",
        with = "edr_eth::serde::sequence"
    )]
    SetNextBlockBaseFeePerGas(
        /// `QUANTITY` - The base fee per gas in wei.
        U128,
    ),
    /// # `hardhat_setNonce`
    ///
    /// Modifies the nonce of an account. The new nonce must be greater than
    /// or equal to the existing nonce.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000001",
    ///     "0xa"
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - This changes the state without mining a block, but instead is tracked
    ///   outside of the blockchain.
    /// - The mempool will be updated to reflect the new nonce, so that
    ///   transactions with a nonce lower than the new nonce will be dropped
    ///   from the pool.
    #[serde(rename = "hardhat_setNonce")]
    SetNonce(
        /// `DATA, 20 bytes` - The address whose nonce to set.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `QUANTITY` - The new nonce value.
        #[serde(
            deserialize_with = "crate::requests::serde::deserialize_nonce",
            serialize_with = "alloy_serde::quantity::serialize"
        )]
        u64,
    ),
    /// # `hardhat_setPrevRandao`
    ///
    /// Sets the `PREVRANDAO` value of the next block.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000000000000000000000000000001"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - Only works for post-merge hardforks. Calling this on a pre-merge
    ///   hardfork will return an error.
    #[serde(rename = "hardhat_setPrevRandao", with = "edr_eth::serde::sequence")]
    SetPrevRandao(
        /// `DATA, 32 bytes` - The `PREVRANDAO` value for the next block.
        B256,
    ),
    /// # `hardhat_setStorageAt`
    ///
    /// Modifies a single storage slot of an account.
    ///
    /// ## Result
    ///
    /// `Boolean` - Always returns `true`.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": [
    ///     "0x0000000000000000000000000000000000000001",
    ///     "0x0",
    ///     "0x0000000000000000000000000000000000000000000000000000000000000001"
    ///   ]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    ///
    /// ## Implementation details
    ///
    /// - This changes the state without mining a block, but instead is tracked
    ///   outside of the blockchain.
    #[serde(rename = "hardhat_setStorageAt")]
    SetStorageAt(
        /// `DATA, 20 bytes` - The address of the account.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_address")]
        Address,
        /// `QUANTITY` - The storage slot index.
        #[serde(deserialize_with = "crate::requests::serde::deserialize_storage_key")]
        U256,
        /// `DATA, 32 bytes` - The new storage value. Must be exactly 32
        /// bytes.
        #[serde(with = "crate::requests::serde::storage_value")]
        U256,
    ),
    /// # `hardhat_stopImpersonatingAccount`
    ///
    /// Stops impersonating an account previously impersonated via
    /// `hardhat_impersonateAccount`.
    ///
    /// ## Result
    ///
    /// `Boolean` - `true` if the account was being impersonated, `false`
    /// otherwise.
    ///
    /// ## Example
    ///
    /// **Request:**
    ///
    /// ```json
    /// {
    ///   "params": ["0x0000000000000000000000000000000000000001"]
    /// }
    /// ```
    ///
    /// **Response:**
    ///
    /// ```json
    /// true
    /// ```
    #[serde(
        rename = "hardhat_stopImpersonatingAccount",
        with = "edr_eth::serde::sequence"
    )]
    StopImpersonatingAccount(
        /// `DATA, 20 bytes` - The address to stop impersonating.
        RpcAddress,
    ),
}

impl<ChainSpecT: RpcChainSpec> MethodInvocation<ChainSpecT> {
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
            MethodInvocation::GetProof(_, _, _) => "eth_getProof",
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
            MethodInvocation::DropTransaction(_) => "hardhat_dropTransaction",
            MethodInvocation::GetAutomine(_) => "hardhat_getAutomine",
            MethodInvocation::ImpersonateAccount(_) => "hardhat_impersonateAccount",
            MethodInvocation::Metadata(_) => "hardhat_metadata",
            MethodInvocation::Mine(_, _) => "hardhat_mine",
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

/// An input that can be either a single `u64` or an array of two `u64` values.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum IntervalConfig {
    /// A fixed value; or disabled, when zero.
    FixedOrDisabled(u64),
    /// An array of two `u64` values representing a `[min, max]` range.
    Range([u64; 2]),
}
