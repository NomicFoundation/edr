#![cfg(feature = "test-utils")]

//! EIP-8024: Backward compatible SWAPN, DUPN and EXCHANGE.
//! see <https://eips.ethereum.org/EIPS/eip-8024>
//!
//! From Amsterdam onward, the DUPN (`0xe6`), SWAPN (`0xe7`) and EXCHANGE
//! (`0xe8`) opcodes manipulate the stack beyond the reach of DUP16/SWAP16,
//! each taking a one-byte immediate operand in a backward-compatible encoding.
//! On earlier hardforks the opcodes are undefined.

use core::str::FromStr as _;

use edr_chain_l1::rpc::call::L1CallRequest;
use edr_primitives::{address, Address, Bytes, U256};
use edr_provider::{
    test_utils::deploy_contract, MethodInvocation, ProviderError, ProviderRequest,
    TransactionFailureReason,
};

use crate::common::{
    bytecode::{
        opcode::{DUPN, EXCHANGE, POP, SWAPN},
        BytecodeBuilder,
    },
    provider::{new_provider, new_provider_with_config},
};

const SENDER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

/// Marker value each contract plants on the stack and must return.
const MARKER: u8 = 42;

/// DUPN/SWAPN immediate decoding to operand value 17, the smallest the
/// backward-compatible encoding can express. Like DUP1/SWAP1, the two opcodes
/// read it off by one: DUPN duplicates the 17th stack item, while SWAPN swaps
/// the top with the item 17 below it (the 18th).
const OPERAND_17_IMMEDIATE: u8 = 0x80;

/// EXCHANGE immediate decoding to the (2nd, 3rd) stack item pair.
const SECOND_AND_THIRD_IMMEDIATE: u8 = 0x8e;

fn contracts() -> [(&'static str, Bytes); 3] {
    // The marker followed by 16 zeros, making it the 17th stack item; DUPN
    // duplicates it to the top.
    let mut dupn = BytecodeBuilder::default();
    dupn.push1(MARKER);
    for _ in 0..16 {
        dupn.push1(0);
    }
    dupn.opcode_with_immediate(DUPN, OPERAND_17_IMMEDIATE)
        .return_top_word();

    // The marker, 16 zeros and a 7 on top, making the marker the 18th stack
    // item; SWAPN swaps it with the top.
    let mut swapn = BytecodeBuilder::default();
    swapn.push1(MARKER);
    for _ in 0..16 {
        swapn.push1(0);
    }
    swapn
        .push1(7)
        .opcode_with_immediate(SWAPN, OPERAND_17_IMMEDIATE)
        .return_top_word();

    // The marker, a 7 and a 0 on top; EXCHANGE swaps the 2nd and 3rd items and
    // POP drops the untouched top, leaving the marker there.
    let mut exchange = BytecodeBuilder::default();
    exchange
        .push1(MARKER)
        .push1(7)
        .push1(0)
        .opcode_with_immediate(EXCHANGE, SECOND_AND_THIRD_IMMEDIATE)
        .opcode(POP)
        .return_top_word();

    [
        ("DUPN", dupn.deployable()),
        ("SWAPN", swapn.deployable()),
        ("EXCHANGE", exchange.deployable()),
    ]
}

fn call_request(contract_address: Address) -> L1CallRequest {
    L1CallRequest {
        from: Some(SENDER),
        to: Some(contract_address),
        ..L1CallRequest::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn opcodes_available_from_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;

    for (opcode, contract) in contracts() {
        let contract_address = deploy_contract(&provider, SENDER, contract)?;

        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::Call(call_request(contract_address), None, None),
        ))?;

        let call_result: String = response.deserialize_result()?;
        let returned_value = U256::from_str(&call_result)?;

        assert_eq!(
            returned_value,
            U256::from(MARKER),
            "{opcode} should retrieve the marker from the stack on Amsterdam"
        );
    }

    Ok(())
}

// Before Amsterdam the opcodes are undefined, so executing them must fail
// rather than return a value.
#[tokio::test(flavor = "multi_thread")]
async fn opcodes_unavailable_before_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider_with_config(|config| {
        config.hardfork = edr_chain_l1::Hardfork::Osaka;
        // Surface the resulting halt as an error instead of empty output.
        config.bail_on_call_failure = true;
    })?;

    for (opcode, contract) in contracts() {
        // The init bytecode never executes the new opcodes (it only copies the
        // runtime out), so deployment succeeds even pre-Amsterdam.
        let contract_address = deploy_contract(&provider, SENDER, contract)?;

        let result = provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            call_request(contract_address),
            None,
            None,
        )));

        // The opcodes are recognized by revm but gated on the hardfork, so they
        // halt with `NotActivated` rather than a generic failure.
        assert!(
            matches!(
                &result,
                Err(ProviderError::TransactionFailed(failure))
                    if matches!(
                        failure.failure.reason,
                        TransactionFailureReason::Inner(edr_chain_l1::HaltReason::NotActivated)
                    )
            ),
            "{opcode} should be inactive before Amsterdam, got {result:?}"
        );
    }

    Ok(())
}
