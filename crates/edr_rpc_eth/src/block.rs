use std::fmt::Debug;

use edr_eth::{block::BlobGas, withdrawal::Withdrawal, Address, Bloom, Bytes, B256, B64, U256};
use serde::{Deserialize, Serialize};

use crate::spec::GetBlockNumber;

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
    /// The commitment hash calculated for a list of [EIP-7685] data requests.
    ///
    /// [EIP-7685]: https://eips.ethereum.org/EIPS/eip-7685
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests_hash: Option<B256>,
}

impl<TransactionT> GetBlockNumber for Block<TransactionT> {
    fn number(&self) -> Option<u64> {
        self.number
    }
}

/// Error that occurs when trying to convert the JSON-RPC `Block` type.
#[derive(Debug, thiserror::Error)]
pub enum MissingFieldError {
    /// Missing hash
    #[error("Missing hash")]
    Hash,
    /// Missing miner
    #[error("Missing miner")]
    Miner,
    /// Missing mix hash
    #[error("Missing mix hash")]
    MixHash,
    /// Missing nonce
    #[error("Missing nonce")]
    Nonce,
    /// Missing number
    #[error("Missing numbeer")]
    Number,
}

impl<TransactionT> TryFrom<&Block<TransactionT>> for edr_eth::block::Header {
    type Error = MissingFieldError;

    fn try_from(value: &Block<TransactionT>) -> Result<Self, Self::Error> {
        let header = edr_eth::block::Header {
            parent_hash: value.parent_hash,
            ommers_hash: value.sha3_uncles,
            beneficiary: value.miner.ok_or(MissingFieldError::Miner)?,
            state_root: value.state_root,
            transactions_root: value.transactions_root,
            receipts_root: value.receipts_root,
            logs_bloom: value.logs_bloom,
            difficulty: value.difficulty,
            number: value.number.ok_or(MissingFieldError::Number)?,
            gas_limit: value.gas_limit,
            gas_used: value.gas_used,
            timestamp: value.timestamp,
            extra_data: value.extra_data.clone(),
            mix_hash: value.mix_hash.ok_or(MissingFieldError::MixHash)?,
            nonce: value.nonce.ok_or(MissingFieldError::Nonce)?,
            base_fee_per_gas: value.base_fee_per_gas,
            withdrawals_root: value.withdrawals_root,
            blob_gas: value.blob_gas_used.and_then(|gas_used| {
                value.excess_blob_gas.map(|excess_gas| BlobGas {
                    gas_used,
                    excess_gas,
                })
            }),
            parent_beacon_block_root: value.parent_beacon_block_root,
            requests_hash: value.requests_hash,
        };

        Ok(header)
    }
}
