//! Ethereum account types

use alloy_rlp::{RlpDecodable, RlpEncodable};
use edr_primitives::{B256, KECCAK_EMPTY, KECCAK_NULL_RLP, U256};
pub use revm_state::{Account, AccountInfo, AccountStatus};

/// Basic account type.
#[derive(Clone, Debug, PartialEq, Eq, RlpDecodable, RlpEncodable)]
pub struct BasicAccount {
    /// Nonce of the account.
    pub nonce: u64,
    /// Balance of the account.
    pub balance: U256,
    /// Storage root of the account.
    pub storage_root: B256,
    /// Code hash of the account.
    pub code_hash: B256,
}

impl Default for BasicAccount {
    fn default() -> Self {
        BasicAccount {
            balance: U256::ZERO,
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            storage_root: KECCAK_NULL_RLP,
        }
    }
}

impl From<BasicAccount> for AccountInfo {
    fn from(account: BasicAccount) -> Self {
        Self {
            balance: account.balance,
            nonce: account.nonce,
            code_hash: account.code_hash,
            code: None,
        }
    }
}

impl From<(&AccountInfo, B256)> for BasicAccount {
    fn from((account_info, storage_root): (&AccountInfo, B256)) -> Self {
        Self {
            nonce: account_info.nonce,
            balance: account_info.balance,
            storage_root,
            code_hash: account_info.code_hash,
        }
    }
}
