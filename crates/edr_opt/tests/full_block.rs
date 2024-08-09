#![cfg(feature = "test-remote")]

use edr_evm::impl_full_block_tests;
use edr_optimism::OptimismChainSpec;
use edr_test_utils::env::get_alchemy_url;

impl_full_block_tests! {
    // This test often failes with an internal Alchemy error:
    // > execution aborted (timeout = 10s)
    //
    // Potentially the block is too old?
    // mainnet_pre_bedrock => OptimismChainSpec {
    //     block_number: 98_235_064,
    //     chain_id: 10,
    //     url: get_alchemy_url().replace("eth-", "opt-"),
    // },
    mainnet_regolith => OptimismChainSpec {
        block_number: 105_235_064,
        chain_id: 10,
        url: get_alchemy_url().replace("eth-", "opt-"),
    },
    mainnet_canyon => OptimismChainSpec {
        block_number: 115_235_064,
        chain_id: 10,
        url: get_alchemy_url().replace("eth-", "opt-"),
    },
    mainnet_ecotone => OptimismChainSpec {
        block_number: 121_874_088,
        chain_id: 10,
        url: get_alchemy_url().replace("eth-", "opt-"),
    },
    mainnet_fjord => OptimismChainSpec {
        block_number: 122_514_212,
        chain_id: 10,
        url: get_alchemy_url().replace("eth-", "opt-"),
    },
}
