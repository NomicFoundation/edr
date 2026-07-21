#![cfg(feature = "test-utils")]

//! EIP-7843: SLOTNUM opcode.
//! see <https://eips.ethereum.org/EIPS/eip-7843>
//
//! From Amsterdam onward, the block header carries a `slotNumber` field and the
//! `SLOTNUM` (`0x4b`) opcode returns it. On earlier hardforks the field is
//! omitted from the RPC response. EDR has no consensus layer, so it simulates
//! one slot per mined block.

use std::{str::FromStr as _, sync::Arc};

use edr_chain_l1::{
    rpc::{block::L1RpcBlock, call::L1CallRequest},
    L1ChainSpec,
};
use edr_eth::BlockSpec;
use edr_primitives::{address, bytes, Address, Bytes, B256, U256};
use edr_provider::{
    test_utils::{create_test_config, deploy_contract, get_latest_block, mine_block},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderError, ProviderRequest,
    TransactionFailureReason,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

const SLOT_NUMBER_JSON_KEY: &str = "slotNumber";

const SENDER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

/// Init bytecode for a contract that returns the current block's slot number.
///
/// `SLOTNUM` (`0x4b`) has no Solidity/Yul builtin, so this is hand-assembled
/// instead of compiled. The deployed runtime is `4b60005260206000f3`
/// (`SLOTNUM; PUSH1 0; MSTORE; PUSH1 0x20; PUSH1 0; RETURN`): it writes the
/// slot number to memory and returns it as a 32-byte word. The `6009600c…f3`
/// prefix is the standard constructor that copies that runtime out as the
/// deployed code.
// TODO: once Solidity exposes SLOTNUM, replace this hand-assembled bytecode
// with a compiled Solidity source for readability.
const SLOT_NUMBER_CONTRACT: Bytes = bytes!("0x6009600c60003960096000f34b60005260206000f3");

fn new_provider(hardfork: edr_chain_l1::Hardfork) -> anyhow::Result<Provider<L1ChainSpec>> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config();
    config.hardfork = hardfork;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    Ok(provider)
}

#[tokio::test(flavor = "multi_thread")]
async fn block_header_includes_slot_number_on_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::AMSTERDAM)?;

    mine_block(&provider);
    let block_json = get_latest_block(&provider);

    assert!(
        block_json.get(SLOT_NUMBER_JSON_KEY).is_some(),
        "Amsterdam block header should include {SLOT_NUMBER_JSON_KEY}, block: {block_json}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn block_header_omits_slot_number_before_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::OSAKA)?;

    mine_block(&provider);
    let block_json = get_latest_block(&provider);

    assert!(
        block_json.get(SLOT_NUMBER_JSON_KEY).is_none(),
        "pre-Amsterdam block header should not include {SLOT_NUMBER_JSON_KEY}. block: {block_json}"
    );

    let block: L1RpcBlock<B256> = serde_json::from_value(block_json)?;
    assert_eq!(
        block.slot_number, None,
        "pre-Amsterdam slot_number should be absent"
    );

    Ok(())
}

// EDR has no consensus layer, so in local mode it anchors the slot number at 0
// for the initial block and simulates one slot per mined block afterwards.
#[tokio::test(flavor = "multi_thread")]
async fn slot_number_increments_per_block() -> anyhow::Result<()> {
    const MINED_BLOCKS: u64 = 5;

    let provider = new_provider(edr_chain_l1::Hardfork::AMSTERDAM)?;

    // Before mining anything, the latest block is the genesis block.
    let genesis: L1RpcBlock<B256> = serde_json::from_value(get_latest_block(&provider))?;
    assert_eq!(
        genesis.slot_number,
        Some(0),
        "the initial block should have slot number 0 in local mode"
    );

    for _ in 0..MINED_BLOCKS {
        mine_block(&provider);
    }

    let latest: L1RpcBlock<B256> = serde_json::from_value(get_latest_block(&provider))?;
    assert_eq!(
        latest.slot_number,
        Some(MINED_BLOCKS),
        "after mining {MINED_BLOCKS} blocks the slot number should be {MINED_BLOCKS}"
    );

    Ok(())
}

// `hardhat_mine` fast-forwards by reserving a run of gap-fill blocks. A user
// who then mines another block must see the slot number continue rather than
// reset: each block in between advances it by exactly one.
#[tokio::test(flavor = "multi_thread")]
async fn slot_number_continues_after_reserved_blocks() -> anyhow::Result<()> {
    const FORWARDED_BLOCKS: u64 = 10;
    let provider = new_provider(edr_chain_l1::Hardfork::AMSTERDAM)?;

    // Mine a couple of blocks, then record the last block before reserving.
    mine_block(&provider);
    mine_block(&provider);
    let before: L1RpcBlock<B256> = serde_json::from_value(get_latest_block(&provider))?;
    let before_slot = before
        .slot_number
        .expect("Amsterdam block should include a slot number");

    // Reserve a run of gap-fill blocks (see `MINIMUM_RESERVABLE_BLOCKS`).
    provider.handle_request(ProviderRequest::with_single(MethodInvocation::Mine(
        Some(FORWARDED_BLOCKS),
        None,
    )))?;

    // Explicitly mine a block on top of the reservation.
    mine_block(&provider);

    let after: L1RpcBlock<B256> = serde_json::from_value(get_latest_block(&provider))?;
    let after_slot = after
        .slot_number
        .expect("Amsterdam block should include a slot number");

    // Every block since (reserved or not) advances the slot number by one.
    assert_eq!(
        after_slot,
        before_slot + FORWARDED_BLOCKS + 1,
        "slot number must continue across reserved blocks, not reset"
    );

    Ok(())
}

// The SLOTNUM opcode (0x4b) must return the executing block's slot number.
#[tokio::test(flavor = "multi_thread")]
async fn slotnum_opcode_returns_block_slot_number() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::AMSTERDAM)?;

    let contract_address = deploy_contract(&provider, SENDER, SLOT_NUMBER_CONTRACT.clone())?;

    // Mine a few more blocks so the call executes in a block well after deployment.
    mine_block(&provider);
    mine_block(&provider);

    let block: L1RpcBlock<B256> = serde_json::from_value(get_latest_block(&provider))?;
    let block_number = block.number.expect("mined block should have a number");
    let slot_number = block
        .slot_number
        .expect("Amsterdam block should include a slot number");

    let response =
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            L1CallRequest {
                from: Some(SENDER),
                to: Some(contract_address),
                ..L1CallRequest::default()
            },
            Some(BlockSpec::Number(block_number)),
            None,
        )))?;

    let call_result: String = serde_json::from_value(response.result)?;
    let slotnum_opcode_returned_value = U256::from_str(&call_result)?;

    assert_eq!(
        slotnum_opcode_returned_value,
        U256::from(slot_number),
        "SLOTNUM should return the executing block's slot number"
    );

    Ok(())
}

// Before Amsterdam the SLOTNUM opcode (0x4b) is undefined, so executing it must
// fail rather than return a value.
#[tokio::test(flavor = "multi_thread")]
async fn slotnum_opcode_unavailable_before_amsterdam() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config();
    config.hardfork = edr_chain_l1::Hardfork::OSAKA;
    // Surface the resulting halt as an error instead of empty output.
    config.bail_on_call_failure = true;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    // The init bytecode never executes 0x4b (it only copies the runtime out), so
    // deployment succeeds even pre-Amsterdam.
    let contract_address = deploy_contract(&provider, SENDER, SLOT_NUMBER_CONTRACT.clone())?;

    let result = provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
        L1CallRequest {
            from: Some(SENDER),
            to: Some(contract_address),
            ..L1CallRequest::default()
        },
        None,
        None,
    )));

    // The opcode is recognized by revm but gated on the hardfork, so it halts with
    // `NotActivated` rather than a generic failure.
    assert!(
        matches!(
            &result,
            Err(ProviderError::TransactionFailed(failure))
                if matches!(
                    failure.failure.reason,
                    TransactionFailureReason::Inner(edr_chain_l1::HaltReason::NotActivated)
                )
        ),
        "SLOTNUM should be inactive before Amsterdam, got {result:?}"
    );

    Ok(())
}
