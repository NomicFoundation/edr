#![cfg(feature = "test-remote")]

use std::{str::FromStr, sync::Arc};

use edr_chain_l1::L1ChainSpec;
use edr_chain_spec_provider::ProviderChainSpec as _;
use edr_defaults::CACHE_DIR;
use edr_primitives::{Address, U256};
use edr_provider::spec::ForkedBlockchainForChainSpec;
use edr_rpc_eth::client::EthRpcClientForChainSpec;
use edr_state_api::{irregular::IrregularState, AccountModifierFn, StateDebug};
use edr_state_fork::ForkedState;
use edr_test_utils::env::get_alchemy_url;
use edr_utils::random::RandomHashGenerator;
use parking_lot::Mutex;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn issue_336_set_balance_after_forking() -> anyhow::Result<()> {
    const TEST_CONTRACT_ADDRESS: &str = "0x530B7F66914c1E345DF1683eae4536fc7b80660f";
    const DEPLOYMENT_BLOCK_NUMBER: u64 = 5464258;

    let contract_address = Address::from_str(TEST_CONTRACT_ADDRESS).unwrap();

    let rpc_client = EthRpcClientForChainSpec::<L1ChainSpec>::new(
        &get_alchemy_url().replace("mainnet", "sepolia"),
        CACHE_DIR.into(),
        None,
    )?;

    let mut state_root_generator = RandomHashGenerator::with_seed("test");
    let state_root = state_root_generator.generate_next();

    let mut state = ForkedState::new(
        runtime::Handle::current(),
        Arc::new(rpc_client),
        Arc::new(Mutex::new(state_root_generator)),
        DEPLOYMENT_BLOCK_NUMBER,
        state_root,
    );

    state.modify_account(
        contract_address,
        AccountModifierFn::new(Box::new(|balance, _nonce, _code| {
            *balance += U256::from(1);
        })),
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn issue_hh_4974_forking_avalanche_c_chain() -> anyhow::Result<()> {
    const FORK_BLOCK_NUMBER: u64 = 12_508_443;

    let url = "https://coston-api.flare.network/ext/bc/C/rpc";
    let rpc_client = EthRpcClientForChainSpec::<L1ChainSpec>::new(url, CACHE_DIR.into(), None)?;
    let mut irregular_state = IrregularState::default();
    let state_root_generator = Arc::new(Mutex::new(RandomHashGenerator::with_seed("test")));

    let _blockchain = ForkedBlockchainForChainSpec::new(
        edr_chain_l1::Hardfork::default(),
        runtime::Handle::current(),
        Arc::new(rpc_client),
        &mut irregular_state,
        state_root_generator,
        L1ChainSpec::chain_configs(),
        Some(FORK_BLOCK_NUMBER),
        None,
    )
    .await?;

    Ok(())
}
