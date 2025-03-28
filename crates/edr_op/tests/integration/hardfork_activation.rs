use edr_eth::BlockSpec;
use edr_op::{OpChainSpec, OpSpecId};
use edr_provider::test_utils::ProviderTestFixture;

use crate::integration::{mainnet_url, sepolia_url};

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
    mainnet: mainnet_url() => edr_op::MAINNET_CHAIN_ID,
    sepolia: sepolia_url() => edr_op::SEPOLIA_CHAIN_ID,
}

#[test]
fn sepolia_hardfork_activations() -> anyhow::Result<()> {
    const CANYON_BLOCK_NUMBER: u64 = 4_089_330;

    let url = sepolia_url();
    let fixture = ProviderTestFixture::<OpChainSpec>::new_forked(Some(url))?;

    let block_spec = BlockSpec::Number(CANYON_BLOCK_NUMBER);
    let config = fixture
        .provider_data
        .create_evm_config_at_block_spec(&block_spec)?;

    assert_eq!(config.spec, OpSpecId::CANYON);

    Ok(())
}
