use std::{str::FromStr, sync::Arc};

use edr_chain_l1::{rpc::call::L1CallRequest, L1ChainSpec};
use edr_primitives::{Address, Bytes, HashMap, U256};
use edr_provider::{
    test_utils::create_test_config_with_fork, time::CurrentTime, ForkConfig, MethodInvocation,
    NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::env::json_rpc_url_provider;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn issue_324() -> anyhow::Result<()> {
    // contract Foo {
    //   uint public x = 1;
    //   uint public y = 2;
    // }
    const TEST_CONTRACT_ADDRESS: &str = "0x530B7F66914c1E345DF1683eae4536fc7b80660f";
    const DEPLOYMENT_BLOCK_NUMBER: u64 = 5464258;

    let contract_address = Address::from_str(TEST_CONTRACT_ADDRESS).unwrap();

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(Some(ForkConfig {
        block_number: Some(DEPLOYMENT_BLOCK_NUMBER),
        cache_dir: edr_defaults::CACHE_DIR.into(),
        chain_overrides: HashMap::default(),
        http_headers: None,
        url: json_rpc_url_provider::ethereum_sepolia(),
    }));
    config.hardfork = edr_chain_l1::Hardfork::CANCUN;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let x = provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
        L1CallRequest {
            to: Some(contract_address),
            data: Some(Bytes::from_str("0x0c55699c").unwrap()), // x()
            ..L1CallRequest::default()
        },
        None,
        None,
    )))?;

    assert_eq!(
        x.result,
        "0x0000000000000000000000000000000000000000000000000000000000000001"
    );

    let y = provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
        L1CallRequest {
            to: Some(contract_address),
            data: Some(Bytes::from_str("0xa56dfe4a").unwrap()), // y()
            ..L1CallRequest::default()
        },
        None,
        None,
    )))?;

    assert_eq!(
        y.result,
        "0x0000000000000000000000000000000000000000000000000000000000000002"
    );

    let x_storage_index = U256::ZERO;
    let expected_x = "0x0000000000000000000000000000000000000000000000000000000000000002";
    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SetStorageAt(
            contract_address,
            x_storage_index,
            U256::from_str(expected_x).unwrap(),
        ),
    ))?;

    let new_x = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetStorageAt(contract_address, x_storage_index, None),
    ))?;

    assert_eq!(new_x.result, expected_x);

    let new_x = provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
        L1CallRequest {
            to: Some(contract_address),
            data: Some(Bytes::from_str("0x0c55699c").unwrap()), // x()
            ..L1CallRequest::default()
        },
        None,
        None,
    )))?;

    assert_eq!(new_x.result, expected_x);

    let y_storage_index = U256::from(1u64);
    let expected_y = "0x0000000000000000000000000000000000000000000000000000000000000003";
    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SetStorageAt(
            contract_address,
            y_storage_index,
            U256::from_str(expected_y).unwrap(),
        ),
    ))?;

    let new_y = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetStorageAt(contract_address, y_storage_index, None),
    ))?;

    assert_eq!(new_y.result, expected_y);

    let new_y = provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
        L1CallRequest {
            to: Some(contract_address),
            data: Some(Bytes::from_str("0xa56dfe4a").unwrap()), // y()
            ..L1CallRequest::default()
        },
        None,
        None,
    )))?;

    assert_eq!(new_y.result, expected_y);

    Ok(())
}
