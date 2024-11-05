use edr_eth::BlockSpec;
use edr_optimism::{OptimismChainSpec, OptimismSpecId};
use edr_provider::test_utils::ProviderTestFixture;
use edr_test_utils::env::get_alchemy_url;

#[test]
fn sepolia_hardfork_activations() -> anyhow::Result<()> {
    const CANYON_BLOCK_NUMBER: u64 = 4_089_330;
    const SEPOLIA_CHAIN_ID: u64 = 11_155_420;

    let url = get_alchemy_url()
        .replace("eth-", "opt-")
        .replace("mainnet", "sepolia");

    let fixture = ProviderTestFixture::<OptimismChainSpec>::new_forked(Some(url))?;

    let block_spec = BlockSpec::Number(CANYON_BLOCK_NUMBER);
    let (_, hardfork) = fixture
        .provider_data
        .create_evm_config_at_block_spec(&block_spec)?;

    assert_eq!(hardfork, OptimismSpecId::CANYON);

    let chain_id = fixture.provider_data.chain_id_at_block_spec(&block_spec)?;
    assert_eq!(chain_id, SEPOLIA_CHAIN_ID);

    Ok(())
}
