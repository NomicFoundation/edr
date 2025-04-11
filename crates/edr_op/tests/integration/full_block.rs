#![cfg(feature = "test-remote")]

use edr_evm::impl_full_block_tests;
use edr_op::OpChainSpec;

use super::op::mainnet_url;

impl_full_block_tests! {
    mainnet_regolith => OpChainSpec {
        block_number: 105_235_064,
        url: mainnet_url(),
    },
    mainnet_canyon => OpChainSpec {
        block_number: 115_235_064,
        url: mainnet_url(),
    },
    mainnet_ecotone => OpChainSpec {
        block_number: 121_874_088,
        url: mainnet_url(),
    },
    mainnet_fjord => OpChainSpec {
        block_number: 122_514_212,
        url: mainnet_url(),
    },
    mainnet_granite => OpChainSpec {
        block_number: 125_235_823,
        url: mainnet_url(),
    },
    mainnet_holocene => OpChainSpec {
        block_number: 130_423_412,
        url: mainnet_url(),
    },
}
