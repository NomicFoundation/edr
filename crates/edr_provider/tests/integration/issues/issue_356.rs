use std::{str::FromStr, sync::Arc};

use anyhow::Context;
use edr_chain_l1::{rpc::call::L1CallRequest, L1ChainSpec};
use edr_primitives::{Address, Bytes, HashMap};
use edr_provider::{
    test_utils::create_test_config_with_fork, time::CurrentTime, ForkConfig, MethodInvocation,
    NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::env::json_rpc_url_provider;
use sha3::{Digest, Keccak256};
use tokio::runtime;

// Check that there is no panic when calling a forked blockchain where the
// hardfork is specified as Cancun, but the block number is before the Cancun
// hardfork. https://github.com/NomicFoundation/edr/issues/356
#[tokio::test(flavor = "multi_thread")]
async fn issue_356() -> anyhow::Result<()> {
    // ERC-20 contract
    const TEST_CONTRACT_ADDRESS: &str = "0xaa8e23fb1079ea71e0a56f48a2aa51851d8433d0";

    let contract_address = Address::from_str(TEST_CONTRACT_ADDRESS).context("Invalid address")?;

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(Some(ForkConfig {
        // Pre-cancun Sepolia block
        block_number: Some(4243456),
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

    let selector = Bytes::copy_from_slice(
        &Keccak256::new_with_prefix("decimals()")
            .finalize()
            .as_slice()[..4],
    );

    let response =
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            L1CallRequest {
                to: Some(contract_address),
                data: Some(selector),
                ..L1CallRequest::default()
            },
            None,
            None,
        )))?;

    assert_eq!(
        response.result,
        "0x0000000000000000000000000000000000000000000000000000000000000006"
    );

    Ok(())
}
