use std::fmt::Debug;

use edr_eth::{withdrawal::Withdrawal, Address, Bloom, Bytes, B256, B64, U256};
use serde::{Deserialize, Serialize};

use crate::chain_spec::GetBlockNumber;

/// block object returned by `eth_getBlockBy*`
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Block<TransactionT> {
    /// Hash of the block
    pub hash: Option<B256>,
    /// hash of the parent block.
    pub parent_hash: B256,
    /// SHA3 of the uncles data in the block
    pub sha3_uncles: B256,
    /// the root of the final state trie of the block
    pub state_root: B256,
    /// the root of the transaction trie of the block
    pub transactions_root: B256,
    /// the root of the receipts trie of the block
    pub receipts_root: B256,
    /// the block number. None when its pending block.
    #[serde(with = "edr_eth::serde::optional_u64")]
    pub number: Option<u64>,
    /// the total used gas by all transactions in this block
    #[serde(with = "edr_eth::serde::u64")]
    pub gas_used: u64,
    /// the maximum gas allowed in this block
    #[serde(with = "edr_eth::serde::u64")]
    pub gas_limit: u64,
    /// the "extra data" field of this block
    pub extra_data: Bytes,
    /// the bloom filter for the logs of the block
    pub logs_bloom: Bloom,
    /// the unix timestamp for when the block was collated
    #[serde(with = "edr_eth::serde::u64")]
    pub timestamp: u64,
    /// integer of the difficulty for this blocket
    pub difficulty: U256,
    /// integer of the total difficulty of the chain until this block
    pub total_difficulty: Option<U256>,
    /// Array of uncle hashes
    #[serde(default)]
    pub uncles: Vec<B256>,
    /// Array of transaction objects, or 32 Bytes transaction hashes depending
    /// on the last given parameter
    #[serde(default)]
    pub transactions: Vec<TransactionT>,
    /// the length of the RLP encoding of this block in bytes
    #[serde(with = "edr_eth::serde::u64")]
    pub size: u64,
    /// Mix hash. None when it's a pending block.
    pub mix_hash: Option<B256>,
    /// hash of the generated proof-of-work. null when its pending block.
    pub nonce: Option<B64>,
    /// base fee per gas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_fee_per_gas: Option<U256>,
    /// the address of the beneficiary to whom the mining rewards were given
    #[serde(skip_serializing_if = "Option::is_none")]
    pub miner: Option<Address>,
    /// withdrawals
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdrawals: Option<Vec<Withdrawal>>,
    /// withdrawals root
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdrawals_root: Option<B256>,
    /// The total amount of gas used by the transactions.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "edr_eth::serde::optional_u64"
    )]
    pub blob_gas_used: Option<u64>,
    /// A running total of blob gas consumed in excess of the target, prior to
    /// the block.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "edr_eth::serde::optional_u64"
    )]
    pub excess_blob_gas: Option<u64>,
    /// Root of the parent beacon block
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_beacon_block_root: Option<B256>,
}

impl<TransactionT> GetBlockNumber for Block<TransactionT> {
    fn number(&self) -> Option<u64> {
        self.number
    }
}
