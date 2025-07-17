use edr_eth::block::{self, HeaderOverrides};
use edr_provider::test_utils::header_overrides;

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
