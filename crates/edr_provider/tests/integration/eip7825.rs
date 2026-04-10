#![cfg(feature = "test-utils")]

use std::sync::Arc;

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_chain_spec::EvmSpecId;
use edr_defaults::SECRET_KEYS;
use edr_primitives::address;
use edr_provider::{
    handlers::{RpcMethodCall, RpcRequest},
    test_utils::create_test_config,
    time::CurrentTime,
    NoopLogger, Provider, ResponseWithCallTraces,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::secret_key::secret_key_to_address;
use parking_lot::RwLock;
use tokio::runtime;

const TRANSACTION_GAS_CAP: u64 = 50_000;
const EXCEEDS_TRANSACTION_GAS_LIMIT: u64 = TRANSACTION_GAS_CAP + 1;

fn new_provider(
    auto_mine: bool,
    transaction_gas_cap: u64,
) -> anyhow::Result<Provider<L1ChainSpec>> {
    let mut config = create_test_config();
    config.hardfork = EvmSpecId::OSAKA;
    config.transaction_gas_cap = Some(transaction_gas_cap);
    config.mining.auto_mine = auto_mine;

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
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

fn send_transaction(
    provider: &Provider<L1ChainSpec>,
    gas_limit: u64,
) -> anyhow::Result<ResponseWithCallTraces> {
    let caller = secret_key_to_address(SECRET_KEYS[0])?;
    let transaction = TransactionRequest {
        from: caller,
        to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
        gas: Some(gas_limit),
        ..TransactionRequest::default()
    };

    let request = RpcMethodCall::with_params("eth_sendTransaction", transaction)?;
    let response = provider.handle_request(RpcRequest::with_single(request))?;

    Ok(response)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_call() -> anyhow::Result<()> {
    let provider = new_provider(false, TRANSACTION_GAS_CAP)?;

    let caller = secret_key_to_address(SECRET_KEYS[0])?;
    let call = L1CallRequest {
        from: Some(caller),
        to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
        gas: Some(EXCEEDS_TRANSACTION_GAS_LIMIT),
        ..L1CallRequest::default()
    };

    let request = RpcMethodCall::with_params("eth_call", (call, Option::<edr_eth::BlockSpec>::None, Option::<edr_rpc_eth::StateOverrideOptions>::None))?;
    let result = provider.handle_request(RpcRequest::with_single(request));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("TxGasLimitGreaterThanCap") || err_msg.contains("gas limit"),
        "Unexpected error: {err_msg}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_estimate_gas() -> anyhow::Result<()> {
    let provider = new_provider(false, TRANSACTION_GAS_CAP)?;

    let caller = secret_key_to_address(SECRET_KEYS[0])?;
    let call = L1CallRequest {
        from: Some(caller),
        to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
        gas: Some(EXCEEDS_TRANSACTION_GAS_LIMIT),
        ..L1CallRequest::default()
    };

    let request = RpcMethodCall::with_params("eth_estimateGas", (call, Option::<edr_eth::BlockSpec>::None))?;
    let result = provider.handle_request(RpcRequest::with_single(request));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("TxGasLimitGreaterThanCap") || err_msg.contains("gas limit"),
        "Unexpected error: {err_msg}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_transaction_exceeds_transaction_cap_with_auto_mine() -> anyhow::Result<()> {
    let provider = new_provider(true, TRANSACTION_GAS_CAP)?;

    let result = send_transaction(&provider, EXCEEDS_TRANSACTION_GAS_LIMIT);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("ExceedsTransactionGasCap") || err_msg.contains("gas cap"),
        "Unexpected error: {err_msg}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_transaction_exceeds_transaction_cap_without_auto_mine() -> anyhow::Result<()> {
    let provider = new_provider(false, TRANSACTION_GAS_CAP)?;

    let result = send_transaction(&provider, EXCEEDS_TRANSACTION_GAS_LIMIT);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("ExceedsTransactionGasCap") || err_msg.contains("gas cap"),
        "Unexpected error: {err_msg}"
    );

    Ok(())
}
