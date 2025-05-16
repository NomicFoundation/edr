#![cfg(feature = "test-utils")]

use std::sync::Arc;

use edr_eth::{U256, address, bytes, l1::L1ChainSpec, signature::public_key_to_address};
use edr_provider::{
    MethodInvocation, NoopLogger, Provider, ProviderRequest, test_utils::create_test_config,
    time::CurrentTime,
};
use edr_rpc_eth::CallRequest;
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas() -> anyhow::Result<()> {
    let mut config = create_test_config();

    let from = {
        let secret_key = config
            .owned_accounts
            .first_mut()
            .expect("should have an account");

        let address = public_key_to_address(secret_key.public_key());

        let account = config
            .genesis_state
            .get_mut(&address)
            .expect("Account should be present in genesis state");

        // Lower the balance to zero. This should not trigger an `OutOfFunds` error in
        // REVM when estimating gas.
        account.balance = Some(U256::from(0u64));

        address
    };

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let _response =
        provider.handle_request(ProviderRequest::Single(MethodInvocation::EstimateGas(
            CallRequest {
                from: Some(from),
                to: Some(address!("0xdf951d2061b12922bfbf22cb17b17f3b39183570")),
                data: Some(bytes!("0x3e6fec0490f79bf6eb2c4f870365e785982e1f101e93b906")),
                ..CallRequest::default()
            },
            None,
        )))?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas_with_value() -> anyhow::Result<()> {
    let value = U256::from(0xau64);

    let mut config = create_test_config();

    let from = {
        let secret_key = config
            .owned_accounts
            .first_mut()
            .expect("should have an account");

        let address = public_key_to_address(secret_key.public_key());

        let account = config
            .genesis_state
            .get_mut(&address)
            .expect("Account should be present in genesis state");

        // Lower the balance to zero. This should not trigger an `OutOfFunds` error in
        // REVM when estimating gas.
        account.balance = Some(U256::from(0u64));

        address
    };

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let _response =
        provider.handle_request(ProviderRequest::Single(MethodInvocation::EstimateGas(
            CallRequest {
                from: Some(from),
                to: Some(address!("0xdf951d2061b12922bfbf22cb17b17f3b39183570")),
                value: Some(value),
                data: Some(bytes!("0x3e6fec0490f79bf6eb2c4f870365e785982e1f101e93b906")),
                ..CallRequest::default()
            },
            None,
        )))?;

    Ok(())
}
