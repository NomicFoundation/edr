#![cfg(any(test, feature = "test-utils"))]

use std::num::NonZeroU64;

use edr_primitives::{Address, HashMap};
use edr_state_api::{account::AccountInfo, StateError};
use edr_state_persistent_trie::{PersistentAccountAndStorageTrie, PersistentStateTrie};

use crate::{MemPool, MemPoolAddTransactionError};

/// A test fixture for `MemPool`.
pub struct MemPoolTestFixture {
    /// The mem pool.
    pub mem_pool: MemPool<edr_chain_l1::L1SignedTransaction>,
    /// The state.
    pub state: PersistentStateTrie,
}

impl MemPoolTestFixture {
    /// Constructs an instance with the provided accounts.
    pub fn with_accounts(accounts: &[(Address, AccountInfo)]) -> Self {
        let accounts = accounts.iter().cloned().collect::<HashMap<_, _>>();
        let trie = PersistentAccountAndStorageTrie::with_accounts(&accounts);

        MemPoolTestFixture {
            // SAFETY: literal is non-zero
            mem_pool: MemPool::new(unsafe { NonZeroU64::new_unchecked(10_000_000u64) }),
            state: PersistentStateTrie::with_accounts_and_storage(trie),
        }
    }

    /// Tries to add the provided transaction to the mem pool.
    pub fn add_transaction(
        &mut self,
        transaction: edr_chain_l1::L1SignedTransaction,
    ) -> Result<(), MemPoolAddTransactionError<StateError>> {
        self.mem_pool.add_transaction(&self.state, transaction)
    }

    /// Sets the block gas limit.
    pub fn set_block_gas_limit(&mut self, block_gas_limit: NonZeroU64) -> Result<(), StateError> {
        self.mem_pool
            .set_block_gas_limit(&self.state, block_gas_limit)
    }

    /// Updates the mem pool.
    pub fn update(&mut self) -> Result<(), StateError> {
        self.mem_pool.update(&self.state)
    }
}
