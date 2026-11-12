#![warn(missing_docs)]

//! Types related to EIP-7825.

use revm_primitives::hardfork::SpecId as EvmSpecId;

/// The per-transaction gas cap set by EIP-7825 at Osaka.
pub const OSAKA_TRANSACTION_GAS_CAP: u64 = alloy_eips::eip7825::MAX_TX_GAS_LIMIT_OSAKA;

/// Returns the per-transaction gas cap introduced by EIP-7825, or `None` for
/// hardforks that predate it.
pub fn transaction_gas_cap_for_hardfork<HardforkT: Into<EvmSpecId>>(
    hardfork: HardforkT,
) -> Option<u64> {
    if hardfork.into() >= EvmSpecId::OSAKA {
        Some(OSAKA_TRANSACTION_GAS_CAP)
    } else {
        None
    }
}
