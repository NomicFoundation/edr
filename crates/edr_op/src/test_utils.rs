use edr_block_header::{BlockHeader, HeaderOverrides};
use edr_provider::test_utils::header_overrides;
use op_revm::OpSpecId;

/// OP default header overrides after Isthmus hardfork
pub fn isthmus_header_overrides(replay_header: &BlockHeader) -> HeaderOverrides<OpSpecId> {
    HeaderOverrides {
        // EDR does not compute the `requests_hash`, as full support for EIP-7685 introduced in
        // Prague is not implemented.
        requests_hash: replay_header.requests_hash,
        // Since Isthmus OP chains overrides the `withdrawals_root` field in the header with the
        // storage root of the L2-to-L1 message passer contract, which EDR does not
        // calculate for forked blockchains.
        withdrawals_root: replay_header.withdrawals_root,
        ..header_overrides(replay_header)
    }
}

/// OP default header overrides after Jovian hardfork
pub fn jovian_header_overrides(replay_header: &BlockHeader) -> HeaderOverrides<OpSpecId> {
    HeaderOverrides {
        // Jovian overrides the `blob_gas` value field since it is now repurposed to
        // store the DA footprint
        blob_gas: replay_header.blob_gas.clone(),
        ..isthmus_header_overrides(replay_header)
    }
}
