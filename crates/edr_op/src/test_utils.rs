use edr_block_header::{BlockHeader, HeaderOverrides};
use edr_provider::test_utils::header_overrides;
use op_revm::OpSpecId;

/// Isthmus overrides the `withdrawals_root` field in the header with the
/// storage root of the L2-to-L1 message passer contract, which EDR does not
/// calculate for forked blockchains.
pub fn isthmus_header_overrides(replay_header: &BlockHeader) -> HeaderOverrides<OpSpecId> {
    HeaderOverrides {
        withdrawals_root: replay_header.withdrawals_root,
        ..header_overrides(replay_header)
    }
}
