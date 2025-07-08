use edr_eth::block::{self, HeaderOverrides};

/// Default header overrides for replaying OP blocks.
pub fn header_overrides(replay_header: &block::Header) -> HeaderOverrides {
    HeaderOverrides {
        beneficiary: Some(replay_header.beneficiary),
        gas_limit: Some(replay_header.gas_limit),
        mix_hash: Some(replay_header.mix_hash),
        parent_beacon_block_root: replay_header.parent_beacon_block_root,
        state_root: Some(replay_header.state_root),
        timestamp: Some(replay_header.timestamp),
        ..HeaderOverrides::default()
    }
}

/// Post-Holocene it's possible for the base fee parameters to be set
/// dynamically using L1 parameters. As EDR doesn't support this yet, we
/// override the base fee with the one from the replayed header.
pub fn custom_base_fee_header_overrides(replay_header: &block::Header) -> HeaderOverrides {
    HeaderOverrides {
        base_fee: replay_header.base_fee_per_gas,
        ..header_overrides(replay_header)
    }
}

/// Isthmus overrides the `withdrawals_root` field in the header with the
/// storage root of the L2-to-L1 message passer contract, which EDR does not
/// calculate for forked blockchains.
pub fn isthmus_header_overrides(replay_header: &block::Header) -> HeaderOverrides {
    HeaderOverrides {
        withdrawals_root: replay_header.withdrawals_root,
        ..header_overrides(replay_header)
    }
}
