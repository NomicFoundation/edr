#![cfg(feature = "test-remote")]

use std::sync::Arc;

use edr_chain_l1::rpc::call::L1CallRequest;
use edr_chain_spec_rpc::RpcChainSpec;
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use edr_eth::{BlockSpec, PreEip1898BlockSpec};
use edr_op::{Hardfork, OpChainSpec};
use edr_primitives::{address, bytes, Address, HashMap, U64};
use edr_provider::{
    test_utils::{create_test_config, create_test_config_with_fork, ProviderTestFixture},
    time::CurrentTime,
    ForkConfig, MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::env::json_rpc_url_provider;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn sepolia_call_with_remote_chain_id() -> anyhow::Result<()> {
    const GAS_PRICE_ORACLE_L1_BLOCK_ADDRESS: Address =
        address!("420000000000000000000000000000000000000F");

    let logger = Box::new(NoopLogger::<OpChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(Some(ForkConfig {
        block_number: None,
        cache_dir: edr_defaults::CACHE_DIR.into(),
        chain_overrides: HashMap::default(),
        http_headers: None,
        url: json_rpc_url_provider::op_sepolia(),
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
            L1CallRequest {
                from: Some(address!("f39fd6e51aad88f6f4ce6ab8827279cfffb92266")),
                to: Some(GAS_PRICE_ORACLE_L1_BLOCK_ADDRESS),
                data: Some(data),
                ..L1CallRequest::default()
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
    op_mainnet: json_rpc_url_provider::op_mainnet() => edr_op::hardfork::op::MAINNET_CHAIN_ID,
    op_sepolia: json_rpc_url_provider::op_sepolia() => edr_op::hardfork::op::SEPOLIA_CHAIN_ID,
    base_mainnet: json_rpc_url_provider::base_mainnet() => edr_op::hardfork::base::MAINNET_CHAIN_ID,
    base_sepolia: json_rpc_url_provider::base_sepolia() => edr_op::hardfork::base::SEPOLIA_CHAIN_ID,
}

#[tokio::test(flavor = "multi_thread")]
async fn custom_base_fee_params() -> anyhow::Result<()> {
    const GAS_PRICE_ORACLE_L1_BLOCK_ADDRESS: Address =
        address!("420000000000000000000000000000000000000F");

    let logger = Box::new(NoopLogger::<OpChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config();
    config.hardfork = Hardfork::HOLOCENE;
    config.base_fee_params = Some(BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![(
        BaseFeeActivation::BlockNumber(0),
        ConstantBaseFeeParams {
            max_change_denominator: 300,
            elasticity_multiplier: 6,
        },
    )])));
    config.chain_id = 10; // op mainnet

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let data = bytes!(
        "de26c4a10000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002c02ea827a6981c4843b9aca00843b9c24e382520994f39fd6e51aad88f6f4ce6ab8827279cfffb922660180c00000000000000000000000000000000000000000"
    );
    let _response =
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            L1CallRequest {
                from: Some(address!("f39fd6e51aad88f6f4ce6ab8827279cfffb92266")),
                to: Some(GAS_PRICE_ORACLE_L1_BLOCK_ADDRESS),
                data: Some(data),
                ..L1CallRequest::default()
            },
            None,
            None,
        )))?;

    let last_block = {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::GetBlockByNumber(
                PreEip1898BlockSpec::Tag(edr_eth::BlockTag::Latest),
                false,
            ),
        ))?;
        serde_json::from_value::<
            edr_chain_l1::rpc::Block<<OpChainSpec as RpcChainSpec>::RpcTransaction>,
        >(response.result)?
    };
    let block_base_fee_params = edr_op::block::decode_base_params(&last_block.extra_data);

    // assert that the block was built using the given configuration values
    assert_eq!(block_base_fee_params.max_change_denominator, 300);
    assert_eq!(block_base_fee_params.elasticity_multiplier, 6);

    Ok(())
}
