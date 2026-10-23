use std::sync::Arc;

use edr_op::OpChainSpec;
use edr_primitives::{address, Address, Bytes, U256};
use edr_provider::{
    test_utils::{create_test_config_with, MinimalProviderConfig},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

const ACCOUNT: Address = address!("0x00000000000000000000000000000000000000aa");

fn provider() -> anyhow::Result<Provider<OpChainSpec>> {
    let config = create_test_config_with(MinimalProviderConfig::local_empty());

    Provider::new(
        runtime::Handle::current(),
        Box::new(NoopLogger::<OpChainSpec>::default()),
        Box::new(|_event| {}),
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )
    .map_err(Into::into)
}

#[tokio::test(flavor = "multi_thread")]
async fn dump_and_load_state() -> anyhow::Result<()> {
    let source = provider()?;
    source.handle_request(ProviderRequest::with_single(
        MethodInvocation::SetStorageAt(ACCOUNT, U256::from(1), U256::from(7)),
    ))?;
    let dump: Bytes = source
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::DumpState(()),
        ))?
        .deserialize_result()?;

    let target = provider()?;
    let loaded: bool = target
        .handle_request(ProviderRequest::with_single(MethodInvocation::LoadState(
            dump,
        )))?
        .deserialize_result()?;
    assert!(loaded);

    let value: Bytes = target
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::GetStorageAt(ACCOUNT, U256::from(1), None),
        ))?
        .deserialize_result()?;
    assert_eq!(U256::from_be_slice(&value), U256::from(7));

    Ok(())
}
