#![cfg(feature = "test-remote")]

use std::sync::Arc;

use edr_chain_l1::rpc::call::L1CallRequest;
use edr_eth::BlockSpec;
use edr_op::OpChainSpec;
use edr_primitives::{address, bytes, Address, HashMap, U64};
use edr_provider::{
    test_utils::{create_test_config_with, MinimalProviderConfig, ProviderTestFixture},
    time::CurrentTime,
    ForkConfig, MethodInvocation, NoopLogger, Provider, ProviderConfig, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::env::json_rpc_url_provider;
use op_revm::OpSpecId;
use tokio::runtime;

fn create_op_provider(config: ProviderConfig<OpSpecId>) -> anyhow::Result<Provider<OpChainSpec>> {
    let logger = Box::new(NoopLogger::<OpChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )
    .map_err(Into::into)
}
#[tokio::test(flavor = "multi_thread")]
async fn sepolia_call_with_remote_chain_id() -> anyhow::Result<()> {
    const GAS_PRICE_ORACLE_L1_BLOCK_ADDRESS: Address =
        address!("420000000000000000000000000000000000000F");

    let mut config =
        create_test_config_with(MinimalProviderConfig::fork_with_accounts(ForkConfig {
            block_number: None,
            cache_dir: edr_defaults::CACHE_DIR.into(),
            chain_overrides: HashMap::default(),
            http_headers: None,
            url: json_rpc_url_provider::op_sepolia(),
        }));

    // Set a different chain ID than the forked chain ID
    config.chain_id = 31337;

    let provider = create_op_provider(config)?;

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

mod base_fee_params {

    use edr_chain_l1::rpc::{block::L1RpcBlock, TransactionRequest};
    use edr_defaults::SECRET_KEYS;
    use edr_eip1559::{
        BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams,
    };
    use edr_eth::PreEip1898BlockSpec;
    use edr_op::Hardfork;
    use edr_primitives::B256;
    use edr_provider::test_utils::create_test_config;
    use edr_test_utils::secret_key::secret_key_to_address;

    use super::*;

    fn trigger_mining_block(provider: &Provider<OpChainSpec>) -> anyhow::Result<()> {
        let caller = secret_key_to_address(SECRET_KEYS[0])?;
        let callee = secret_key_to_address(SECRET_KEYS[1])?;
        let transaction = TransactionRequest {
            from: caller,
            to: Some(callee),
            ..TransactionRequest::default()
        };
        let _result = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(transaction),
        ))?;

        Ok(())
    }

    fn latest_block(provider: &Provider<OpChainSpec>) -> anyhow::Result<L1RpcBlock<B256>> {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::GetBlockByNumber(
                PreEip1898BlockSpec::Tag(edr_eth::BlockTag::Latest),
                false,
            ),
        ))?;
        serde_json::from_value::<L1RpcBlock<B256>>(response.result).map_err(Into::into)
    }

    mod local {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn custom_base_fee_params() -> anyhow::Result<()> {
            let mut config = create_test_config();
            config.hardfork = Hardfork::HOLOCENE;
            config.base_fee_params =
                Some(BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![(
                    BaseFeeActivation::BlockNumber(0),
                    ConstantBaseFeeParams {
                        max_change_denominator: 300,
                        elasticity_multiplier: 6,
                    },
                )])));

            let provider = create_op_provider(config)?;
            trigger_mining_block(&provider)?;
            let latest_block = latest_block(&provider)?;

            let block_base_fee_params = edr_op::block::decode_base_params(&latest_block.extra_data);

            // assert that the block was built using the given configuration values
            assert_eq!(block_base_fee_params.max_change_denominator, 300);
            assert_eq!(block_base_fee_params.elasticity_multiplier, 6);

            Ok(())
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn multiple_custom_base_fee_params() -> anyhow::Result<()> {
            let mut config = create_test_config();
            config.hardfork = Hardfork::HOLOCENE;
            config.base_fee_params = Some(BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
                (
                    BaseFeeActivation::BlockNumber(0),
                    ConstantBaseFeeParams {
                        max_change_denominator: 300,
                        elasticity_multiplier: 6,
                    },
                ),
                (
                    BaseFeeActivation::BlockNumber(3),
                    ConstantBaseFeeParams {
                        max_change_denominator: 200,
                        elasticity_multiplier: 2,
                    },
                ),
            ])));

            let provider = create_op_provider(config)?;

            trigger_mining_block(&provider)?;
            let block = latest_block(&provider)?;
            assert_eq!(block.number, Some(1));
            let block_base_fee_params = edr_op::block::decode_base_params(&block.extra_data);

            // assert that the block was built using the given configuration values
            assert_eq!(block_base_fee_params.max_change_denominator, 300);
            assert_eq!(block_base_fee_params.elasticity_multiplier, 6);

            trigger_mining_block(&provider)?;

            let block = latest_block(&provider)?;
            assert_eq!(block.number, Some(2));
            let block_base_fee_params = edr_op::block::decode_base_params(&block.extra_data);

            // Header extra_data encodes base_fee_params values needed for calculating next
            // block. As this is the block number 2, and we configured new
            // values from block 3, this block header should already encode the
            // new values
            assert_eq!(block_base_fee_params.max_change_denominator, 200);
            assert_eq!(block_base_fee_params.elasticity_multiplier, 2);
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn provider_uses_chain_default_base_fee_params() -> anyhow::Result<()> {
            let mut config = create_test_config();
            config.hardfork = Hardfork::ISTHMUS;
            config.chain_id = edr_op::hardfork::op::MAINNET_CHAIN_ID;

            let provider = create_op_provider(config)?;

            trigger_mining_block(&provider)?;
            let latest_block = latest_block(&provider)?;

            let block_base_fee_params = edr_op::block::decode_base_params(&latest_block.extra_data);

            // Defaults to CANYON values since when creating a new local blockchain block
            // number will be 0, so the dynamic configs won't apply yet, and EDR
            // will fallback to the most recent Hardfork-defined params
            assert_eq!(block_base_fee_params.max_change_denominator, 250);
            assert_eq!(block_base_fee_params.elasticity_multiplier, 6);
            Ok(())
        }
    }

    #[cfg(feature = "test-remote")]
    mod fork {
        use edr_test_utils::env::json_rpc_url_provider;

        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn custom_base_fee_params() -> anyhow::Result<()> {
            let mut config =
                create_test_config_with(MinimalProviderConfig::fork_with_accounts(ForkConfig {
                    block_number: None,
                    cache_dir: edr_defaults::CACHE_DIR.into(),
                    chain_overrides: HashMap::default(),
                    http_headers: None,
                    url: json_rpc_url_provider::op_mainnet(),
                }));
            config.hardfork = Hardfork::ISTHMUS;
            config.base_fee_params =
                Some(BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![(
                    BaseFeeActivation::BlockNumber(0),
                    ConstantBaseFeeParams {
                        max_change_denominator: 300,
                        elasticity_multiplier: 6,
                    },
                )])));
            config.chain_id = edr_op::hardfork::op::MAINNET_CHAIN_ID;

            let provider = create_op_provider(config)?;

            trigger_mining_block(&provider)?;

            let latest_block = latest_block(&provider)?;
            let block_base_fee_params = edr_op::block::decode_base_params(&latest_block.extra_data);

            // assert that the block was built using the given configuration values
            assert_eq!(block_base_fee_params.max_change_denominator, 300);
            assert_eq!(block_base_fee_params.elasticity_multiplier, 6);

            Ok(())
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn multiple_custom_base_fee_params() -> anyhow::Result<()> {
            let fork_block_number = 135_513_416;
            let mut config =
                create_test_config_with(MinimalProviderConfig::fork_with_accounts(ForkConfig {
                    block_number: Some(fork_block_number),
                    cache_dir: edr_defaults::CACHE_DIR.into(),
                    chain_overrides: HashMap::default(),
                    http_headers: None,
                    url: json_rpc_url_provider::op_mainnet(),
                }));
            config.hardfork = Hardfork::ISTHMUS;
            config.base_fee_params = Some(BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
                (
                    BaseFeeActivation::BlockNumber(fork_block_number),
                    ConstantBaseFeeParams {
                        max_change_denominator: 300,
                        elasticity_multiplier: 6,
                    },
                ),
                (
                    BaseFeeActivation::BlockNumber(fork_block_number + 3),
                    ConstantBaseFeeParams {
                        max_change_denominator: 200,
                        elasticity_multiplier: 2,
                    },
                ),
            ])));

            let provider = create_op_provider(config)?;

            trigger_mining_block(&provider)?;
            let block = latest_block(&provider)?;
            assert_eq!(block.number, Some(fork_block_number + 1));
            let block_base_fee_params = edr_op::block::decode_base_params(&block.extra_data);

            // assert that the block was built using the first configuration values
            assert_eq!(block_base_fee_params.max_change_denominator, 300);
            assert_eq!(block_base_fee_params.elasticity_multiplier, 6);

            trigger_mining_block(&provider)?;

            let block = latest_block(&provider)?;
            assert_eq!(block.number, Some(fork_block_number + 2));
            let block_base_fee_params = edr_op::block::decode_base_params(&block.extra_data);

            // Header extra_data encodes base_fee_params values needed for calculating next
            // block. As this is the block number `fork_block_number + 2`, and
            // we configured new values from block `fork_block_number + 3`, this
            // block header should already encode the new values
            assert_eq!(block_base_fee_params.max_change_denominator, 200);
            assert_eq!(block_base_fee_params.elasticity_multiplier, 2);
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn provider_uses_chain_default_base_fee_params() -> anyhow::Result<()> {
            let first_dynamic_base_fee_activation = 135_513_416;
            let mut config =
                create_test_config_with(MinimalProviderConfig::fork_with_accounts(ForkConfig {
                    block_number: Some(first_dynamic_base_fee_activation),
                    cache_dir: edr_defaults::CACHE_DIR.into(),
                    chain_overrides: HashMap::default(),
                    http_headers: None,
                    url: json_rpc_url_provider::op_mainnet(),
                }));
            config.hardfork = Hardfork::HOLOCENE;
            config.chain_id = edr_op::hardfork::op::MAINNET_CHAIN_ID;

            let provider = create_op_provider(config)?;

            trigger_mining_block(&provider)?;
            let latest_block = latest_block(&provider)?;

            let block_base_fee_params = edr_op::block::decode_base_params(&latest_block.extra_data);

            // assert that the block was built using OP_MAINNET values
            // `first_dynamic_base_fee_activation` block number matches with the third base
            // fee activation on `edr_op::hardfork::op::MAINNET_BASE_FEE_PARAMS`
            assert_eq!(block_base_fee_params.max_change_denominator, 250);
            assert_eq!(block_base_fee_params.elasticity_multiplier, 4);
            Ok(())
        }
    }
}
