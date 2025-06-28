#![cfg(feature = "test-remote")]

use edr_eth::block::{self, HeaderOverrides};
use edr_evm::impl_full_block_tests;
use edr_op::OpChainSpec;

use super::op::mainnet_url;

fn pre_dynamic_base_fee_header_overrides(replay_header: &block::Header) -> HeaderOverrides {
    HeaderOverrides {
        beneficiary: Some(replay_header.beneficiary),
        gas_limit: Some(replay_header.gas_limit),
        extra_data: Some(replay_header.extra_data.clone()),
        mix_hash: Some(replay_header.mix_hash),
        nonce: Some(replay_header.nonce),
        parent_beacon_block_root: replay_header.parent_beacon_block_root,
        state_root: Some(replay_header.state_root),
        timestamp: Some(replay_header.timestamp),
        ..HeaderOverrides::default()
    }
}

/// EDR does not support dynamic base fees, so we need to override the
/// `base_fee_per_gas` field in the header with the value from the replayed
/// header.
fn post_dynamic_base_fee_header_overrides(replay_header: &block::Header) -> HeaderOverrides {
    HeaderOverrides {
        base_fee: replay_header.base_fee_per_gas,
        ..pre_dynamic_base_fee_header_overrides(replay_header)
    }
}

/// Isthmus overrides the `withdrawals_root` field in the header with the
/// storage root of the L2-to-L1 message passer contract, which EDR does not
/// calculate for forked blockchains.
fn isthmus_header_overrides(replay_header: &block::Header) -> HeaderOverrides {
    HeaderOverrides {
        withdrawals_root: replay_header.withdrawals_root,
        ..post_dynamic_base_fee_header_overrides(replay_header)
    }
}

impl_full_block_tests! {
    mainnet_regolith => OpChainSpec {
        block_number: 105_235_064,
        url: mainnet_url(),
        header_overrides_constructor: pre_dynamic_base_fee_header_overrides,
    },
    mainnet_canyon => OpChainSpec {
        block_number: 115_235_064,
        url: mainnet_url(),
        header_overrides_constructor: pre_dynamic_base_fee_header_overrides,
    },
    mainnet_ecotone => OpChainSpec {
        block_number: 121_874_088,
        url: mainnet_url(),
        header_overrides_constructor: pre_dynamic_base_fee_header_overrides,
    },
    mainnet_fjord => OpChainSpec {
        block_number: 122_514_212,
        url: mainnet_url(),
        header_overrides_constructor: pre_dynamic_base_fee_header_overrides,
    },
    mainnet_granite => OpChainSpec {
        block_number: 125_235_823,
        url: mainnet_url(),
        header_overrides_constructor: pre_dynamic_base_fee_header_overrides,
    },
    mainnet_holocene => OpChainSpec {
        block_number: 130_423_412,
        url: mainnet_url(),
        header_overrides_constructor: pre_dynamic_base_fee_header_overrides,
    },
    mainnet_137620147 => OpChainSpec {
        block_number: 137_620_147,
        url: mainnet_url(),
        header_overrides_constructor: isthmus_header_overrides,
    },
}
