use edr_eth::block::{self, HeaderOverrides};
use edr_provider::test_utils::header_overrides;
use op_revm::OpSpecId;

/// Since Holocene override `extra_data` field since it can contain a
/// `base_fee_param` update On OP stack chains, when a a `SystemConfig` eip-1559
/// fields update happens, the change gets evidenced first in a block
/// `extra_data` field which will then used for calculating next block
/// `base_fee`
///
/// > Placing the EIP-1559 parameters within the L2 block header allows us to
/// > retain the purity of the function that computes the next block's base fee
/// > from its parent block header, while still allowing them to be dynamically
/// > configured
///
/// see <https://specs.optimism.io/protocol/holocene/exec-engine.html>
pub fn holocene_header_overrides(replay_header: &block::Header) -> HeaderOverrides<OpSpecId> {
    HeaderOverrides {
        extra_data: Some(replay_header.extra_data.clone()),
        ..header_overrides(replay_header)
    }
}

/// Isthmus overrides the `withdrawals_root` field in the header with the
/// storage root of the L2-to-L1 message passer contract, which EDR does not
/// calculate for forked blockchains.
pub fn isthmus_header_overrides(replay_header: &block::Header) -> HeaderOverrides<OpSpecId> {
    HeaderOverrides {
        withdrawals_root: replay_header.withdrawals_root,
        ..holocene_header_overrides(replay_header)
    }
}
