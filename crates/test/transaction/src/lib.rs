//! Test utilities for transaction-related tests.
#![warn(missing_docs)]

use edr_chain_spec::EvmSpecId;
use edr_evm::transaction;
use edr_primitives::{Address, Bytes, U256};
use edr_transaction::TxKind;

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
