use edr_eth::{l1::L1ChainSpec, Address, Bytes, B256, U128, U160, U256};
use edr_provider::MethodInvocation;

use crate::common::help_test_method_invocation_serde;

#[test]
fn serde_hardhat_drop_transaction() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::DropTransaction(
        B256::from(U256::from(1)),
    ));
}

#[test]
fn serde_hardhat_get_automine() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::GetAutomine(()));
}

#[test]
fn serde_hardhat_impersonate_account() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::ImpersonateAccount(
        Address::from(U160::from(1)).into(),
    ));
}

#[test]
fn serde_hardhat_metadata() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::Metadata(()));
}

#[test]
fn serde_hardhat_mine() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::Mine(Some(1), Some(1)));
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::Mine(Some(1), None));
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::Mine(None, Some(1)));
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::Mine(None, None));

    let json = r#"{"jsonrpc":"2.0","method":"hardhat_mine","params":[],"id":2}"#;
    let deserialized: MethodInvocation<L1ChainSpec> = serde_json::from_str(json)
        .unwrap_or_else(|_| panic!("should have successfully deserialized json {json}"));
    assert_eq!(MethodInvocation::Mine(None, None), deserialized);
}

#[test]
fn serde_hardhat_set_balance() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::SetBalance(
        Address::from(U160::from(1)),
        U256::ZERO,
    ));
}

#[test]
fn serde_hardhat_set_code() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::SetCode(
        Address::from(U160::from(1)),
        Bytes::from(&b"whatever"[..]),
    ));
}

#[test]
fn serde_hardhat_set_coinbase() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::SetCoinbase(
        Address::random(),
    ));
}

#[test]
fn serde_hardhat_set_logging_enabled() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::SetLoggingEnabled(true));
}

#[test]
fn serde_hardhat_set_min_gas_price() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::SetMinGasPrice(U128::from(
        1,
    )));
}

#[test]
fn serde_hardhat_set_next_block_base_fee_per_gas() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::SetNextBlockBaseFeePerGas(
        U128::from(1),
    ));
}

#[test]
fn serde_hardhat_set_nonce() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::SetNonce(
        Address::random(),
        1u64,
    ));
}

#[test]
fn serde_hardhat_set_prev_randao() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::SetPrevRandao(
        B256::random(),
    ));
}

#[test]
fn serde_hardhat_set_storage_at() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::SetStorageAt(
        Address::random(),
        U256::ZERO,
        U256::MAX,
    ));
}

#[test]
fn serde_hardhat_stop_impersonating_account() {
    help_test_method_invocation_serde(MethodInvocation::<L1ChainSpec>::StopImpersonatingAccount(
        Address::random().into(),
    ));
}
