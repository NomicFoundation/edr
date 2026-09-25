#![cfg(feature = "test-remote")]

//! EIP-7997: Deterministic Factory Contract.
//! see <https://eips.ethereum.org/EIPS/eip-7997>
//!
//! Forking a pre-Amsterdam block with a local Amsterdam hardfork injects the
//! factory as an irregular-state override at the fork block, replacing the
//! whole account. Ethereum mainnet has had the factory for years, so a
//! pre-Amsterdam local hardfork sees the remote account untouched, with its
//! real nonce.

use edr_blockchain_api::StateAtBlock as _;
use edr_blockchain_fork::eips::eip7997::{
    DETERMINISTIC_FACTORY_ADDRESS, DETERMINISTIC_FACTORY_BYTECODE,
};
use edr_primitives::Bytecode;
use edr_state_api::{account::AccountInfo, irregular::IrregularState};

use crate::common::create_forked_blockchain;

/// Ethereum mainnet block with Osaka active and Amsterdam not yet scheduled.
const POST_OSAKA_BLOCK_NUMBER: u64 = 24_500_000;

async fn factory_account_at_fork_block(
    local_hardfork: edr_chain_l1::Hardfork,
) -> anyhow::Result<(AccountInfo, Bytecode)> {
    let mut irregular_state = IrregularState::default();
    let blockchain = create_forked_blockchain(
        &mut irregular_state,
        POST_OSAKA_BLOCK_NUMBER,
        local_hardfork,
    )
    .await?;

    let state = blockchain
        .state_at_block_number(POST_OSAKA_BLOCK_NUMBER, irregular_state.state_overrides())?;

    let account = state
        .basic(DETERMINISTIC_FACTORY_ADDRESS)?
        .expect("factory account should exist");

    let code = account
        .code
        .clone()
        .map_or_else(|| state.code_by_hash(account.code_hash), Ok)?;

    Ok((account, code))
}

#[tokio::test(flavor = "multi_thread")]
#[serial_test::serial]
async fn forked_pre_amsterdam_with_amsterdam_injects_factory() -> anyhow::Result<()> {
    let (account, code) = factory_account_at_fork_block(edr_chain_l1::Hardfork::Amsterdam).await?;

    assert_eq!(code, Bytecode::new_raw(DETERMINISTIC_FACTORY_BYTECODE));
    assert_eq!(
        account.nonce, 1,
        "the override replaces the remote account with the genesis-insertion nonce"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial_test::serial]
async fn forked_mainnet_pre_amsterdam_with_osaka_keeps_remote_factory() -> anyhow::Result<()> {
    let (account, code) = factory_account_at_fork_block(edr_chain_l1::Hardfork::Osaka).await?;

    assert_eq!(code, Bytecode::new_raw(DETERMINISTIC_FACTORY_BYTECODE));
    assert!(
        account.nonce > 1,
        "the mainnet factory has performed many CREATE2s; its nonce must be preserved, got {}",
        account.nonce
    );

    Ok(())
}
