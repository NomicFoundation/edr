#![cfg(feature = "test-remote")]

use edr_eth::BlockSpec;
use edr_op::OpChainSpec;
use edr_provider::test_utils::ProviderTestFixture;
use edr_test_utils::env::json_rpc_url_provider;

macro_rules! impl_test_hardfork_activation {
    ($($net:ident: $url:expr => {
        $($hardfork:ident: $block_number:literal => $result:expr,)+
    },)+) => {
        $(
            $(
                paste::item! {
                    #[test]
                    fn [<hardfork_activation_ $net _ $hardfork>]() -> anyhow::Result<()> {
                        let url = $url;
                        let fixture = ProviderTestFixture::<OpChainSpec>::new_forked(Some(url))?;

                        let block_spec = BlockSpec::Number($block_number);
                        let config = fixture
                            .provider_data
                            .create_evm_config_at_block_spec(&block_spec)?;

                        assert_eq!(config.spec, $result);

                        if $result > edr_op::Hardfork::Regolith && $block_number > 0 {
                            let parent_block_spec = BlockSpec::Number($block_number - 1);
                            let config = fixture
                                .provider_data
                                .create_evm_config_at_block_spec(&parent_block_spec)?;

                            assert_ne!(config.spec, $result);
                        }

                        Ok(())
                    }
                }
            )+
        )+
    };
}

// Block numbers were determined using `cast find-block <timestamp>`
impl_test_hardfork_activation! {
    op_mainnet: json_rpc_url_provider::op_mainnet() => {
        regolith: 105_235_063 => edr_op::Hardfork::Regolith,
        canyon: 114_696_812 => edr_op::Hardfork::Canyon,
        ecotone: 117_387_812 => edr_op::Hardfork::Ecotone,
        fjord: 122_514_212 => edr_op::Hardfork::Fjord,
        granite: 125_235_812 => edr_op::Hardfork::Granite,
        holocene: 130_423_412 => edr_op::Hardfork::Holocene,
        isthmus: 135_603_812 => edr_op::Hardfork::Isthmus,
    },
    op_sepolia: json_rpc_url_provider::op_sepolia() => {
        regolith: 0 => edr_op::Hardfork::Regolith,
        canyon: 4_089_330 => edr_op::Hardfork::Canyon,
        ecotone: 8_366_130 => edr_op::Hardfork::Ecotone,
        fjord: 12_597_930 => edr_op::Hardfork::Fjord,
        granite: 15_837_930 => edr_op::Hardfork::Granite,
        holocene: 20_415_330 => edr_op::Hardfork::Holocene,
        isthmus: 26_551_530 => edr_op::Hardfork::Isthmus,
    },
}
