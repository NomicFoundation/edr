#![cfg(any(test, feature = "test-utils"))]

use std::num::NonZeroU64;

use edr_chain_spec::EvmSpecId;
use edr_primitives::{Address, Bytes, HashMap, U256};
use edr_state_api::{account::AccountInfo, StateError};
use edr_state_persistent_trie::{PersistentAccountAndStorageTrie, PersistentStateTrie};
use edr_transaction::TxKind;

use crate::{transaction, MemPool, MemPoolAddTransactionError};

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

/// Creates a dummy EIP-155 transaction.
pub fn dummy_eip155_transaction(
    caller: Address,
    nonce: u64,
) -> Result<edr_chain_l1::L1SignedTransaction, transaction::CreationError> {
    dummy_eip155_transaction_with_price(caller, nonce, 0)
}

/// Creates a dummy EIP-155 transaction with the provided gas price.
pub fn dummy_eip155_transaction_with_price(
    caller: Address,
    nonce: u64,
    gas_price: u128,
) -> Result<edr_chain_l1::L1SignedTransaction, transaction::CreationError> {
    dummy_eip155_transaction_with_price_and_limit(caller, nonce, gas_price, 30_000)
}

/// Creates a dummy EIP-155 transaction with the provided gas limit.
pub fn dummy_eip155_transaction_with_limit(
    caller: Address,
    nonce: u64,
    gas_limit: u64,
) -> Result<edr_chain_l1::L1SignedTransaction, transaction::CreationError> {
    dummy_eip155_transaction_with_price_and_limit(caller, nonce, 0, gas_limit)
}

fn dummy_eip155_transaction_with_price_and_limit(
    caller: Address,
    nonce: u64,
    gas_price: u128,
    gas_limit: u64,
) -> Result<edr_chain_l1::L1SignedTransaction, transaction::CreationError> {
    dummy_eip155_transaction_with_price_limit_and_value(
        caller,
        nonce,
        gas_price,
        gas_limit,
        U256::ZERO,
    )
}

/// Creates a dummy EIP-155 transaction with the provided gas price, gas limit,
/// and value.
pub fn dummy_eip155_transaction_with_price_limit_and_value(
    caller: Address,
    nonce: u64,
    gas_price: u128,
    gas_limit: u64,
    value: U256,
) -> Result<edr_chain_l1::L1SignedTransaction, transaction::CreationError> {
    let from = Address::random();
    let request = edr_chain_l1::request::Eip155 {
        nonce,
        gas_price,
        gas_limit,
        kind: TxKind::Call(from),
        value,
        input: Bytes::new(),
        chain_id: 123,
    };
    let transaction = request.fake_sign(caller);
    let transaction = edr_chain_l1::L1SignedTransaction::from(transaction);

    transaction::validate(transaction, EvmSpecId::default())
}

/// Creates a dummy EIP-1559 transaction with the provided max fee and max
/// priority fee per gas.
pub fn dummy_eip1559_transaction(
    caller: Address,
    nonce: u64,
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: u128,
) -> Result<edr_chain_l1::L1SignedTransaction, transaction::CreationError> {
    let from = Address::random();
    let request = edr_chain_l1::request::Eip1559 {
        chain_id: 123,
        nonce,
        max_priority_fee_per_gas,
        max_fee_per_gas,
        gas_limit: 30_000,
        kind: TxKind::Call(from),
        value: U256::ZERO,
        input: Bytes::new(),
        access_list: Vec::new(),
    };
    let transaction = request.fake_sign(caller);
    let transaction = edr_chain_l1::L1SignedTransaction::from(transaction);

    transaction::validate(transaction, EvmSpecId::default())
}
