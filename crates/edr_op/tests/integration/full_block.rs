#![cfg(feature = "test-remote")]

use edr_evm::impl_full_block_tests;
use edr_op::OpChainSpec;

use super::{op, base};

impl_full_block_tests! {
    op_mainnet_regolith => OpChainSpec {
        block_number: 105_235_064,
        url: op::mainnet_url(),
    },
    op_mainnet_canyon => OpChainSpec {
        block_number: 115_235_064,
        url: op::mainnet_url(),
    },
    op_mainnet_ecotone => OpChainSpec {
        block_number: 121_874_088,
        url: op::mainnet_url(),
    },
    op_mainnet_fjord => OpChainSpec {
        block_number: 122_514_212,
        url: op::mainnet_url(),
    },
    op_mainnet_granite => OpChainSpec {
        block_number: 125_235_823,
        url: op::mainnet_url(),
    },
    op_mainnet_holocene => OpChainSpec {
        block_number: 130_423_412,
        url: op::mainnet_url(),
    },

    base_mainnet_holocene => OpChainSpec {
        block_number: 24_828_127,
        url: base::mainnet_url(),
    },
}
