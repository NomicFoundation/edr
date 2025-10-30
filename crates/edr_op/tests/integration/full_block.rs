#![cfg(feature = "test-remote")]

use edr_evm::impl_full_block_tests;
use edr_op::{test_utils::isthmus_header_overrides, OpChainSpec};
use edr_provider::test_utils::header_overrides;

impl_full_block_tests! {
    mainnet_regolith => OpChainSpec {
        block_number: 105_235_064,
        url: super::op::mainnet_url(),
        header_overrides_constructor: header_overrides,
    },
    mainnet_canyon => OpChainSpec {
        block_number: 115_235_064,
        url: super::op::mainnet_url(),
        header_overrides_constructor: header_overrides,
    },
    mainnet_ecotone => OpChainSpec {
        block_number: 121_874_088,
        url: super::op::mainnet_url(),
        header_overrides_constructor: header_overrides,
    },
    mainnet_fjord => OpChainSpec {
        block_number: 122_514_212,
        url: super::op::mainnet_url(),
        header_overrides_constructor: header_overrides,
    },
    mainnet_granite => OpChainSpec {
        block_number: 125_235_823,
        url: super::op::mainnet_url(),
        header_overrides_constructor: header_overrides,
    },
    // The first Holocene block used a dynamic base fee set in the SystemConfig.
    mainnet_holocene => OpChainSpec {
        block_number: 130_423_412,
        url: super::op::mainnet_url(),
        header_overrides_constructor: header_overrides,
    },
    // The second Holocene block should use the dynamic base fee from the parent block's `extra_data`.
    mainnet_holocene_plus_one => OpChainSpec {
        block_number: 130_423_413,
        url: super::op::mainnet_url(),
        header_overrides_constructor: header_overrides,
    },
    mainnet_system_config_update_on_extra_data => OpChainSpec {
        block_number: 135_513_415,
        url: super::op::mainnet_url(),
        header_overrides_constructor: header_overrides,
    },
    mainnet_after_system_config_update => OpChainSpec {
        block_number: 135_513_416,
        url: super::op::mainnet_url(),
        header_overrides_constructor: header_overrides,
    },
    // The Isthmus hardfork modified the GasPriceOracle predeploy in this block
    // but we don't support forked account overrides yet.
    // mainnet_isthmus => OpChainSpec {
    //     block_number: 135_603_812,
    //     url: super::op::mainnet_url(),
    //     header_overrides_constructor: isthmus_header_overrides,
    // },
    mainnet_isthmus_plus_one => OpChainSpec {
        block_number: 135_603_813,
        url: super::op::mainnet_url(),
        header_overrides_constructor: isthmus_header_overrides,
    },
    mainnet_137620147 => OpChainSpec {
        block_number: 137_620_147,
        url: super::op::mainnet_url(),
        header_overrides_constructor: isthmus_header_overrides,
    },
    base_mainnet_37511249 => OpChainSpec {
        block_number: 37_511_249,
        url: super::base::mainnet_url(),
        header_overrides_constructor: isthmus_header_overrides,
    },
}
