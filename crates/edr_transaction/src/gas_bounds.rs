//! Per-transaction gas bounds, by hardfork.

use edr_chain_spec::EvmSpecId;
use revm_primitives::eip7825;

/// Absolute cap on a transaction's gas limit from Amsterdam ([EIP-8037]).
///
/// [EIP-8037]: https://eips.ethereum.org/EIPS/eip-8037
// TODO: replace with revm's constant once
// <https://github.com/bluealloy/revm/issues/3929> is resolved.
pub const TX_MAX_TOTAL_GAS_LIMIT: u64 = u32::MAX as u64;

/// The [EIP-7825] cap, or `None` before Osaka. It bounds `tx.gas` up to Osaka
/// and execution gas only from Amsterdam ([EIP-8037]).
///
/// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
/// [EIP-8037]: https://eips.ethereum.org/EIPS/eip-8037
pub fn execution_gas_bound_for_hardfork<HardforkT: Into<EvmSpecId>>(
    hardfork: HardforkT,
) -> Option<u64> {
    if hardfork.into() >= EvmSpecId::OSAKA {
        Some(eip7825::TX_GAS_LIMIT_CAP)
    } else {
        None
    }
}

/// The bound on a transaction's gas limit, or `None` before Osaka: the
/// [EIP-7825] cap at Osaka, [`TX_MAX_TOTAL_GAS_LIMIT`] from Amsterdam.
///
/// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
pub fn total_gas_bound_for_hardfork<HardforkT: Into<EvmSpecId>>(
    hardfork: HardforkT,
) -> Option<u64> {
    let evm_spec_id = hardfork.into();
    if evm_spec_id >= EvmSpecId::AMSTERDAM {
        Some(TX_MAX_TOTAL_GAS_LIMIT)
    } else if evm_spec_id >= EvmSpecId::OSAKA {
        Some(eip7825::TX_GAS_LIMIT_CAP)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_gas_bound_activates_at_osaka() {
        assert_eq!(execution_gas_bound_for_hardfork(EvmSpecId::PRAGUE), None);
        assert_eq!(
            execution_gas_bound_for_hardfork(EvmSpecId::OSAKA),
            Some(eip7825::TX_GAS_LIMIT_CAP)
        );
        assert_eq!(
            execution_gas_bound_for_hardfork(EvmSpecId::AMSTERDAM),
            Some(eip7825::TX_GAS_LIMIT_CAP)
        );
    }

    #[test]
    fn total_gas_bound_widens_at_amsterdam() {
        assert_eq!(total_gas_bound_for_hardfork(EvmSpecId::PRAGUE), None);
        assert_eq!(
            total_gas_bound_for_hardfork(EvmSpecId::OSAKA),
            Some(eip7825::TX_GAS_LIMIT_CAP)
        );
        assert_eq!(
            total_gas_bound_for_hardfork(EvmSpecId::AMSTERDAM),
            Some(TX_MAX_TOTAL_GAS_LIMIT)
        );
    }
}
