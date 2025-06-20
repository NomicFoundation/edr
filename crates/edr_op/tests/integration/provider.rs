#![cfg(feature = "test-remote")]

use std::sync::Arc;

use edr_eth::{address, bytes, Address, BlockSpec, HashMap, U64};
use edr_op::OpChainSpec;
use edr_provider::{
    test_utils::{create_test_config_with_fork, ProviderTestFixture},
    time::CurrentTime,
    ForkConfig, MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_rpc_eth::CallRequest;
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

use crate::integration::{base, op};

#[tokio::test(flavor = "multi_thread")]
async fn sepolia_call_with_remote_chain_id() -> anyhow::Result<()> {
    const GAS_PRICE_ORACLE_L1_BLOCK_ADDRESS: Address =
        address!("420000000000000000000000000000000000000F");

    let logger = Box::new(NoopLogger::<OpChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(Some(ForkConfig {
        block_number: None,
        cache_dir: edr_defaults::CACHE_DIR.into(),
        chain_overrides: HashMap::new(),
        http_headers: None,
        url: op::sepolia_url(),
    }));

    // Set a different chain ID than the forked chain ID
    config.chain_id = 31337;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let last_block_number = {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::BlockNumber(()),
        ))?;

        serde_json::from_value::<U64>(response.result)?.to::<u64>()
    };

    let data = bytes!(
        "de26c4a10000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002c02ea827a6981c4843b9aca00843b9c24e382520994f39fd6e51aad88f6f4ce6ab8827279cfffb922660180c00000000000000000000000000000000000000000"
    );
    let _response =
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            CallRequest {
                from: Some(address!("f39fd6e51aad88f6f4ce6ab8827279cfffb92266")),
                to: Some(GAS_PRICE_ORACLE_L1_BLOCK_ADDRESS),
                data: Some(data),
                ..CallRequest::default()
            },
            Some(BlockSpec::Number(last_block_number)),
            None,
        )))?;

    Ok(())
}

macro_rules! impl_test_chain_id {
    ($($name:ident: $url:expr => $result:expr,)+) => {
        $(
            paste::item! {
                #[test]
                fn [<chain_id_for_ $name>]() -> anyhow::Result<()> {
                    let url = $url;
                    let fixture = ProviderTestFixture::<OpChainSpec>::new_forked(Some(url))?;

                    let block_spec = BlockSpec::Number(0);
                    let chain_id = fixture.provider_data.chain_id_at_block_spec(&block_spec)?;
                    assert_eq!(chain_id, $result);

                    Ok(())
                }
            }
        )+
    };
}

impl_test_chain_id! {
    op_mainnet: op::mainnet_url() => edr_op::hardfork::op::MAINNET_CHAIN_ID,
    op_sepolia: op::sepolia_url() => edr_op::hardfork::op::SEPOLIA_CHAIN_ID,
    base_mainnet: base::mainnet_url() => edr_op::hardfork::base::MAINNET_CHAIN_ID,
    base_sepolia: base::sepolia_url() => edr_op::hardfork::base::SEPOLIA_CHAIN_ID,
}
