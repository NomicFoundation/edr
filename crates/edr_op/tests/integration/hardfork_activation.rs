#![cfg(feature = "test-remote")]

use edr_eth::BlockSpec;
use edr_op::{OpChainSpec, OpSpecId};
use edr_provider::test_utils::ProviderTestFixture;

use crate::integration::{base, op};

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

                        if $result > OpSpecId::REGOLITH && $block_number > 0 {
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
    op_mainnet: op::mainnet_url() => {
        regolith: 105_235_063 => OpSpecId::REGOLITH,
        canyon: 114_696_812 => OpSpecId::CANYON,
        ecotone: 117_387_812 => OpSpecId::ECOTONE,
        fjord: 122_514_212 => OpSpecId::FJORD,
        granite: 125_235_812 => OpSpecId::GRANITE,
        holocene: 130_423_412 => OpSpecId::HOLOCENE,
    },
    op_sepolia: op::sepolia_url() => {
        regolith: 0 => OpSpecId::REGOLITH,
        canyon: 4_089_330 => OpSpecId::CANYON,
        ecotone: 8_366_130 => OpSpecId::ECOTONE,
        fjord: 12_597_930 => OpSpecId::FJORD,
        granite: 15_837_930 => OpSpecId::GRANITE,
        holocene: 20_415_330 => OpSpecId::HOLOCENE,
    },
    base_mainnet: base::mainnet_url() => {
        regolith: 0 => OpSpecId::REGOLITH,
        canyon: 9_101_527 => OpSpecId::CANYON,
        ecotone: 11_792_527 => OpSpecId::ECOTONE,
        fjord: 16_918_927 => OpSpecId::FJORD,
        granite: 19_640_527 => OpSpecId::GRANITE,
        holocene: 24_828_127 => OpSpecId::HOLOCENE,
    },
    base_sepolia: base::sepolia_url() => {
        regolith: 0 => OpSpecId::REGOLITH,
        canyon: 2_106_456 => OpSpecId::CANYON,
        ecotone: 6_383_256 => OpSpecId::ECOTONE,
        fjord: 10_615_056 => OpSpecId::FJORD,
        granite: 13_855_056 => OpSpecId::GRANITE,
        holocene: 18_432_456 => OpSpecId::HOLOCENE,
    },
}
