//! Helpers for constructing an L1 test provider and issuing common requests.
#![cfg(feature = "test-utils")]

use std::{num::NonZeroU64, sync::Arc};

use edr_chain_l1::{
    rpc::{call::L1CallRequest, receipt::L1RpcTransactionReceipt, TransactionRequest},
    L1ChainSpec,
};
use edr_eth::PreEip1898BlockSpec;
use edr_primitives::{Address, Bytes, B256, U256, U64};
use edr_provider::{
    config::ProviderConfig, test_utils::create_test_config, time::CurrentTime, MethodInvocation,
    NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

/// Creates a provider from the default test config with the given hardfork.
pub fn new_provider(hardfork: edr_chain_l1::Hardfork) -> anyhow::Result<Provider<L1ChainSpec>> {
    new_provider_with_config(|config| config.hardfork = hardfork)
}

/// Creates a provider from the default test config after applying `customize`
/// to it.
pub fn new_provider_with_config(
    customize: impl FnOnce(&mut ProviderConfig<edr_chain_l1::Hardfork>),
) -> anyhow::Result<Provider<L1ChainSpec>> {
    let mut config = create_test_config();
    customize(&mut config);

    new_provider_from_config(config)
}

/// Creates a provider with a no-op logger and subscriber and an empty contract
/// decoder.
pub fn new_provider_from_config(
    config: ProviderConfig<edr_chain_l1::Hardfork>,
) -> anyhow::Result<Provider<L1ChainSpec>> {
    let provider = Provider::new(
        runtime::Handle::current(),
        Box::new(NoopLogger::<L1ChainSpec>::default()),
        Box::new(|_event| {}),
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    Ok(provider)
}

/// Returns the transaction's receipt via `eth_getTransactionReceipt`.
pub fn transaction_receipt(
    provider: &Provider<L1ChainSpec>,
    transaction_hash: B256,
) -> anyhow::Result<L1RpcTransactionReceipt> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionReceipt(transaction_hash),
    ))?;

    let receipt: Option<L1RpcTransactionReceipt> = response.deserialize_result()?;
    receipt.ok_or_else(|| anyhow::anyhow!("receipt should exist"))
}

/// Returns the `gasUsed` from the transaction's receipt.
pub fn gas_used(provider: &Provider<L1ChainSpec>, transaction_hash: B256) -> u64 {
    transaction_receipt(provider, transaction_hash)
        .expect("receipt should exist")
        .gas_used
}

/// Sends the transaction via `eth_sendTransaction` and returns the `gasUsed`
/// of its receipt.
pub fn gas_used_by_sending(
    provider: &Provider<L1ChainSpec>,
    request: TransactionRequest,
) -> anyhow::Result<u64> {
    let transaction_hash = send_transaction(provider, request)?;

    Ok(gas_used(provider, transaction_hash))
}

/// Estimates the request's gas usage via `eth_estimateGas`.
pub fn estimate_gas(provider: &Provider<L1ChainSpec>, request: L1CallRequest) -> u64 {
    let response = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
            request, None,
        )))
        .expect("eth_estimateGas should succeed");

    let gas: U64 = response
        .deserialize_result()
        .expect("response should be U64");

    gas.into_limbs()[0]
}

/// Sends the transaction via `eth_sendTransaction`, returning its hash.
pub fn send_transaction(
    provider: &Provider<L1ChainSpec>,
    request: TransactionRequest,
) -> anyhow::Result<B256> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(request),
    ))?;

    Ok(response.deserialize_result()?)
}

/// Parses the quantity `field` of a JSON-RPC object.
pub fn quantity_field(object: &serde_json::Value, field: &str) -> anyhow::Result<u64> {
    let quantity = object
        .get(field)
        .ok_or_else(|| anyhow::anyhow!("object should have a `{field}` field"))?;
    let quantity: U64 = serde_json::from_value(quantity.clone())?;

    Ok(quantity.to())
}

/// Returns the gas limit of the block at `block_spec`.
pub fn block_gas_limit(
    provider: &Provider<L1ChainSpec>,
    block_spec: PreEip1898BlockSpec,
) -> anyhow::Result<u64> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(block_spec, false),
    ))?;

    let block: Option<serde_json::Value> = response.deserialize_result()?;
    let block = block.ok_or_else(|| anyhow::anyhow!("block should exist"))?;

    quantity_field(&block, "gasLimit")
}

/// Returns the account's balance via `eth_getBalance`.
pub fn balance_at(provider: &Provider<L1ChainSpec>, address: Address) -> anyhow::Result<U256> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBalance(address, None),
    ))?;

    Ok(response.deserialize_result()?)
}

/// Returns the account's code via `eth_getCode`.
pub fn code_at(provider: &Provider<L1ChainSpec>, address: Address) -> anyhow::Result<Bytes> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetCode(address, None),
    ))?;

    Ok(response.deserialize_result()?)
}

/// Returns the account's nonce via `eth_getTransactionCount`.
pub fn nonce_at(provider: &Provider<L1ChainSpec>, address: Address) -> anyhow::Result<U64> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionCount(address, None),
    ))?;

    Ok(response.deserialize_result()?)
}

/// Returns the account's storage slot via `eth_getStorageAt`.
pub fn storage_at(
    provider: &Provider<L1ChainSpec>,
    address: Address,
    index: U256,
) -> anyhow::Result<U256> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetStorageAt(address, index, None),
    ))?;

    Ok(response.deserialize_result()?)
}

/// Above the Osaka cap, but within the Amsterdam bound and the default block
/// gas limit.
pub const DEFAULT_TRANSACTION_GAS_LIMIT_ABOVE_OSAKA_CAP: NonZeroU64 =
    NonZeroU64::new(30_000_000).expect("gas limit should be non-zero");

/// A mainnet block before the Osaka activation, with a gas limit of at least
/// 30M.
#[cfg(feature = "test-remote")]
pub const PRAGUE_MAINNET_BLOCK_NUMBER: u64 = 23_000_000;

/// A mainnet block after the Osaka activation.
#[cfg(feature = "test-remote")]
pub const OSAKA_MAINNET_BLOCK_NUMBER: u64 = 24_000_000;

/// Returns the test config with accounts, forking Ethereum mainnet at
/// `block_number`.
#[cfg(feature = "test-remote")]
pub fn mainnet_fork_config(block_number: u64) -> ProviderConfig<edr_chain_l1::Hardfork> {
    use edr_primitives::HashMap;
    use edr_provider::{
        config::ForkConfig,
        test_utils::{create_test_config_with, MinimalProviderConfig},
    };
    use edr_test_utils::env::json_rpc_url_provider;

    create_test_config_with(MinimalProviderConfig::fork_with_accounts(ForkConfig {
        block_number: Some(block_number),
        cache_dir: edr_defaults::CACHE_DIR.into(),
        chain_overrides: HashMap::default(),
        http_headers: None,
        url: json_rpc_url_provider::ethereum_mainnet(),
    }))
}
