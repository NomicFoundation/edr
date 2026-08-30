//! Helpers for constructing an L1 test provider and issuing common requests.
#![cfg(feature = "test-utils")]

use std::sync::Arc;

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_primitives::B256;
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

/// Sends the transaction via `eth_sendTransaction`, returning its hash.
pub fn send_transaction(
    provider: &Provider<L1ChainSpec>,
    request: TransactionRequest,
) -> anyhow::Result<B256> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(request),
    ))?;

    Ok(serde_json::from_value(response.result)?)
}
