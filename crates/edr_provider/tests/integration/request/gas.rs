use edr_chain_l1::{test_utils::provider, L1ChainSpec};
use edr_eth::{transaction::ExecutableTransaction as _, BlockSpec, BlockTag};
use edr_evm::state::StateOverrides;
use edr_provider::{
    requests::resolve_estimate_gas_request,
    test_utils::{pending_base_fee, ProviderTestFixture},
};
use edr_rpc_eth::CallRequest;

#[test]
fn resolve_estimate_gas_request_with_default_max_priority_fee() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let max_fee_per_gas = pending_base_fee(&mut fixture.provider_data)?.max(10_000_000_000);

    let request = CallRequest {
        from: Some(fixture.nth_local_account(0)?),
        to: Some(fixture.nth_local_account(1)?),
        max_fee_per_gas: Some(max_fee_per_gas),
        ..CallRequest::default()
    };

    let resolved = resolve_estimate_gas_request(
        &mut fixture.provider_data,
        request,
        &BlockSpec::pending(),
        &StateOverrides::default(),
    )?;

    assert_eq!(*resolved.gas_price(), max_fee_per_gas);
    assert_eq!(
        resolved.max_priority_fee_per_gas().cloned(),
        Some(1_000_000_000)
    );

    Ok(())
}

#[test]
fn resolve_estimate_gas_request_with_default_max_fee_when_pending_block() -> anyhow::Result<()> {
    let base_fee = 10u128;
    let max_priority_fee_per_gas = 1u128;

    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;
    fixture
        .provider_data
        .set_next_block_base_fee_per_gas(base_fee)?;

    let request = CallRequest {
        from: Some(fixture.nth_local_account(0)?),
        to: Some(fixture.nth_local_account(1)?),
        max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
        ..CallRequest::default()
    };

    let resolved = resolve_estimate_gas_request(
        &mut fixture.provider_data,
        request,
        &BlockSpec::pending(),
        &StateOverrides::default(),
    )?;

    assert_eq!(
        *resolved.gas_price(),
        2 * base_fee + max_priority_fee_per_gas
    );
    assert_eq!(
        resolved.max_priority_fee_per_gas().cloned(),
        Some(max_priority_fee_per_gas)
    );

    Ok(())
}

#[test]
fn resolve_estimate_gas_request_with_default_max_fee_when_historic_block() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;
    fixture.provider_data.set_next_block_base_fee_per_gas(10)?;

    let transaction = provider::signed_dummy_transaction(&fixture, 0, None)?;
    fixture.provider_data.send_transaction(transaction)?;

    let last_block = fixture.provider_data.last_block()?;
    assert_eq!(last_block.header().number, 1);

    let max_priority_fee_per_gas = 1u128;
    let request = CallRequest {
        from: Some(fixture.nth_local_account(0)?),
        to: Some(fixture.nth_local_account(1)?),
        max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
        ..CallRequest::default()
    };

    let resolved = resolve_estimate_gas_request(
        &mut fixture.provider_data,
        request,
        &BlockSpec::Tag(BlockTag::Latest),
        &StateOverrides::default(),
    )?;

    assert_eq!(
        Some(*resolved.gas_price()),
        last_block
            .header()
            .base_fee_per_gas
            .map(|base_fee| base_fee + max_priority_fee_per_gas)
    );
    assert_eq!(
        resolved.max_priority_fee_per_gas().cloned(),
        Some(max_priority_fee_per_gas)
    );

    Ok(())
}

#[test]
fn resolve_estimate_gas_request_with_capped_max_priority_fee() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;
    fixture.provider_data.set_next_block_base_fee_per_gas(0)?;

    let max_fee_per_gas = 123u128;

    let request = CallRequest {
        from: Some(fixture.nth_local_account(0)?),
        to: Some(fixture.nth_local_account(1)?),
        max_fee_per_gas: Some(max_fee_per_gas),
        ..CallRequest::default()
    };

    let resolved = resolve_estimate_gas_request(
        &mut fixture.provider_data,
        request,
        &BlockSpec::pending(),
        &StateOverrides::default(),
    )?;

    assert_eq!(*resolved.gas_price(), max_fee_per_gas);
    assert_eq!(
        resolved.max_priority_fee_per_gas().cloned(),
        Some(max_fee_per_gas)
    );

    Ok(())
}
