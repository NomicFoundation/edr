#![cfg(feature = "test-utils")]

//! [EIP-8246]: Remove `SELFDESTRUCT` burn.
//!
//! A contract that self-destructs in the transaction that created it is still
//! cleared (nonce, code and storage) but keeps its balance from Amsterdam on.
//! Before Amsterdam the balance is burned and the account removed.
//!
//! [EIP-8246]: https://eips.ethereum.org/EIPS/eip-8246

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_primitives::{address, Address, Bytes, U256, U64};
use edr_provider::{test_utils::transfer_value, Provider};

use crate::common::{
    bytecode::{opcode, BytecodeBuilder},
    provider::{
        balance_at, code_at, new_provider, nonce_at, send_transaction, storage_at,
        transaction_receipt,
    },
};

const SENDER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

const STORAGE_INDEX: U256 = U256::ZERO;

/// Creates a contract whose init code writes a storage slot and then
/// self-destructs to its own address, so the balance it was created with has
/// nowhere else to go. Returns the created address.
fn create_with_value(provider: &Provider<L1ChainSpec>, value: U256) -> anyhow::Result<Address> {
    let mut init_code = BytecodeBuilder::default();
    init_code
        .push1(1)
        .push1(0)
        .opcode(opcode::SSTORE)
        .opcode(opcode::ADDRESS)
        .opcode(opcode::SELFDESTRUCT);

    let transaction_hash = send_transaction(
        provider,
        TransactionRequest {
            from: SENDER,
            data: Some(init_code.runtime()),
            value: Some(value),
            ..TransactionRequest::default()
        },
    )?;

    let receipt = transaction_receipt(provider, transaction_hash)?;
    assert_eq!(receipt.status, Some(true), "creation should succeed");

    Ok(receipt
        .contract_address
        .expect("creation should report the contract address"))
}

#[tokio::test(flavor = "multi_thread")]
async fn keeps_balance_of_contract_selfdestructed_in_creation_at_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;
    let value = U256::from(1000);

    let contract = create_with_value(&provider, value)?;

    assert_eq!(balance_at(&provider, contract)?, value);
    assert_eq!(code_at(&provider, contract)?, Bytes::new());
    assert_eq!(nonce_at(&provider, contract)?, U64::ZERO);
    assert_eq!(storage_at(&provider, contract, STORAGE_INDEX)?, U256::ZERO);

    // The next transaction executes against the persisted balance-only account.
    let top_up = U256::from(500);
    transfer_value(&provider, SENDER, contract, top_up);

    assert_eq!(balance_at(&provider, contract)?, value + top_up);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn burns_balance_of_contract_selfdestructed_in_creation_before_amsterdam(
) -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;

    let contract = create_with_value(&provider, U256::from(1000))?;

    assert_eq!(balance_at(&provider, contract)?, U256::ZERO);
    assert_eq!(code_at(&provider, contract)?, Bytes::new());
    assert_eq!(nonce_at(&provider, contract)?, U64::ZERO);

    Ok(())
}
