//! Types for Ethereum state management

pub mod account;
mod diff;
pub mod r#dyn;
mod error;
pub mod irregular;
mod r#override;

use core::{fmt::Debug, ops::Deref};

use alloy_rpc_types::EIP1186AccountProofResponse;
use auto_impl::auto_impl;
use edr_primitives::{Address, Bytecode, HashMap, StorageKey, B256, U256};
use edr_trie::sec_trie_root;
pub use revm_database_interface::DatabaseCommit as StateCommit;
pub use revm_state::{EvmState, EvmStorage, EvmStorageSlot};

pub use self::{diff::StateDiff, error::StateError, r#dyn::DynState, r#override::StateOverride};
use crate::account::{AccountInfo, BasicAccount};

/// Account storage mapping of indices to values.
pub type AccountStorage = HashMap<U256, U256>;

/// Mapping of addresses to trie state accounts.
pub type EvmTrieState = HashMap<Address, BasicAccount>;

/// Trait for reading state information.
#[auto_impl(&, &mut, Box, Rc, Arc)]
pub trait State {
    /// Combinatorial state error.
    type Error;

    /// Get basic account information.
    fn basic(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error>;

    /// Get account code by its hash
    fn code_by_hash(&self, code_hash: B256) -> Result<Bytecode, Self::Error>;

    /// Get storage value of address at index.
    fn storage(&self, address: Address, index: U256) -> Result<U256, Self::Error>;
}

type BoxedAccountModifierFn = Box<dyn Fn(&mut U256, &mut u64, &mut Option<Bytecode>) + Send>;

/// Debuggable function type for modifying account information.
pub struct AccountModifierFn {
    inner: BoxedAccountModifierFn,
}

impl AccountModifierFn {
    /// Constructs an [`AccountModifierFn`] from the provided function.
    pub fn new(func: BoxedAccountModifierFn) -> Self {
        Self { inner: func }
    }
}

impl Debug for AccountModifierFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            std::any::type_name::<dyn Fn(&mut U256, &mut u64, &mut Option<Bytecode>)>()
        )
    }
}

impl Deref for AccountModifierFn {
    type Target = dyn Fn(&mut U256, &mut u64, &mut Option<Bytecode>);

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

/// A trait for debug operation on a database.
#[auto_impl(&mut, Box)]
pub trait StateDebug {
    /// The state's error type.
    type Error;

    /// Retrieves the storage root of the account at the specified address.
    fn account_storage_root(&self, address: &Address) -> Result<Option<B256>, Self::Error>;

    /// Inserts the provided account at the specified address.
    fn insert_account(
        &mut self,
        address: Address,
        account_info: AccountInfo,
    ) -> Result<(), Self::Error>;

    /// Modifies the account at the specified address using the provided
    /// function.
    ///
    /// Returns the modified (or created) account.
    fn modify_account(
        &mut self,
        address: Address,
        modifier: AccountModifierFn,
    ) -> Result<AccountInfo, Self::Error>;

    /// Removes and returns the account at the specified address, if it exists.
    fn remove_account(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error>;

    /// Serializes the state using ordering of addresses and storage indices.
    fn serialize(&self) -> String;

    /// Sets the storage slot at the specified address and index to the provided
    /// value.
    ///
    /// Returns the old value.
    fn set_account_storage_slot(
        &mut self,
        address: Address,
        index: U256,
        value: U256,
    ) -> Result<U256, Self::Error>;

    /// Retrieves the storage root of the database.
    fn state_root(&self) -> Result<B256, Self::Error>;
}

#[auto_impl(&mut, Box)]
pub trait StateProof {
    /// The state's error type.
    type Error;

    /// Returns the account information together with the Merkle proofs for the
    /// account and its associated storage keys.
    fn proof(
        &self,
        address: Address,
        storage_keys: Vec<StorageKey>,
    ) -> Result<EIP1186AccountProofResponse, Self::Error>;
}

/// Trait for reading state information.
#[auto_impl(&mut, Box)]
pub trait StateMut {
    /// Combinatorial state error.
    type Error;

    /// Get basic account information.
    fn basic_mut(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error>;

    /// Get account code by its hash
    fn code_by_hash_mut(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error>;

    /// Get storage value of address at index.
    fn storage_mut(&mut self, address: Address, index: U256) -> Result<U256, Self::Error>;
}

/*
/// Trait that meets all requirements for a synchronous database
pub trait SyncState<ErrorT: Debug + Send>:
    State<Error = ErrorT> + StateCommit + StateDebug<Error = ErrorT> + Debug + DynClone + Send + Sync
{
}

impl<ErrorT: Debug + Send> Clone for Box<dyn SyncState<ErrorT>> {
    fn clone(&self) -> Self {
        dyn_clone::clone_box(&**self)
    }
}

impl<ErrorT, StateT> SyncState<ErrorT> for StateT
where
    ErrorT: Debug + Send,
    StateT: State<Error = ErrorT>
        + StateCommit
        + StateDebug<Error = ErrorT>
        + Debug
        + DynClone
        + Send
        + Sync,
{
}
*/

/// Calculates the state root hash of the provided state.
pub fn state_root<'a, I>(state: I) -> B256
where
    I: IntoIterator<Item = (&'a Address, &'a BasicAccount)>,
{
    sec_trie_root(state.into_iter().map(|(address, account)| {
        let account = alloy_rlp::encode(account);
        (address, account)
    }))
}

/// Calculates the storage root hash of the provided storage.
pub fn storage_root<'a, I>(storage: I) -> B256
where
    I: IntoIterator<Item = (&'a U256, &'a U256)>,
{
    sec_trie_root(storage.into_iter().map(|(index, value)| {
        let value = alloy_rlp::encode(value);
        (index.to_be_bytes::<32>(), value)
    }))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use edr_primitives::KECCAK_NULL_RLP;

    use super::*;

    #[test]
    fn empty_state_root() {
        let state = EvmTrieState::default();

        assert_eq!(state_root(&state), KECCAK_NULL_RLP);
    }

    #[test]
    fn empty_storage_root() {
        let storage = AccountStorage::default();

        assert_eq!(storage_root(&storage), KECCAK_NULL_RLP);
    }

    #[test]
    fn precompiles_state_root() {
        const EXPECTED: &str = "0x5766c887a7240e4d1c035ccd3830a2f6a0c03d213a9f0b9b27c774916a4abcce";

        let mut state = EvmTrieState::default();

        for idx in 1..=8u8 {
            let mut address = Address::ZERO;
            address.0[19] = idx;
            state.insert(address, BasicAccount::default());
        }

        assert_eq!(state_root(&state), B256::from_str(EXPECTED).unwrap());
    }
}
