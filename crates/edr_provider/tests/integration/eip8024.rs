#![cfg(feature = "test-utils")]

//! EIP-8024: Backward compatible SWAPN, DUPN and EXCHANGE.
//! see <https://eips.ethereum.org/EIPS/eip-8024>
//!
//! From Amsterdam onward, the DUPN (`0xe6`), SWAPN (`0xe7`) and EXCHANGE
//! (`0xe8`) opcodes manipulate the stack beyond the reach of DUP16/SWAP16,
//! each taking a one-byte immediate operand in a backward-compatible encoding

use core::str::FromStr as _;

use edr_chain_l1::rpc::call::L1CallRequest;
use edr_primitives::{address, bytes, Address, Bytes, U256};
use edr_provider::{
    test_utils::deploy_contract, MethodInvocation, ProviderError, ProviderRequest,
    TransactionFailureReason,
};

use crate::common::provider::{new_provider, new_provider_with_config};

const SENDER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

/// Marker value each contract plants on the stack and must return. The stack
/// slots around it hold `0` or `7`, so a wrong stack manipulation returns a
/// different value instead of failing outright.
const MARKER: u64 = 42;

/// Init bytecode for a contract exercising DUPN.
///
/// The runtime pushes the marker followed by 16 zeros, making the marker the
/// 17th stack item, then duplicates it to the top and returns it:
/// `602a` (PUSH1 42), 16 x `6000`, `e680` (DUPN, depth 17), `600052` (MSTORE
/// at 0), `60206000f3` (RETURN 32 bytes). The `602c600c…` prefix is the
/// standard constructor that copies the runtime out as the deployed code.
const DUPN_CONTRACT: Bytes = bytes!(
    "0x602c600c600039602c6000f3602a6000600060006000600060006000600060006000600060006000600060006000e68060005260206000f3"
);

/// Init bytecode for a contract exercising SWAPN.
///
/// The runtime pushes the marker, 16 zeros and a `7` on top, making the marker
/// the 18th stack item, then swaps it with the top and returns it:
/// `602a` (PUSH1 42), 16 x `6000`, `6007` (PUSH1 7), `e780` (SWAPN, swaps the
/// top with the item 17 below it), `600052`, `60206000f3`.
const SWAPN_CONTRACT: Bytes = bytes!(
    "0x602e600c600039602e6000f3602a60006000600060006000600060006000600060006000600060006000600060006007e78060005260206000f3"
);

/// Init bytecode for a contract exercising EXCHANGE.
///
/// The runtime pushes the marker, a `7` and a `0` on top, swaps the 2nd and
/// 3rd items (`e88e`), pops the untouched top (`50`) and returns the new 2nd
/// item: `602a` (PUSH1 42), `6007` (PUSH1 7), `6000` (PUSH1 0), `e88e`
/// (EXCHANGE), `50` (POP), `600052`, `60206000f3`.
const EXCHANGE_CONTRACT: Bytes =
    bytes!("0x6011600c60003960116000f3602a60076000e88e5060005260206000f3");

fn contracts() -> [(&'static str, Bytes); 3] {
    [
        ("DUPN", DUPN_CONTRACT),
        ("SWAPN", SWAPN_CONTRACT),
        ("EXCHANGE", EXCHANGE_CONTRACT),
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

        let call_result: String = serde_json::from_value(response.result)?;
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
