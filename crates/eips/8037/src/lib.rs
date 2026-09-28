#![warn(missing_docs)]

//! Types related to EIP-8037.

use revm_primitives::hardfork::SpecId as EvmSpecId;

/// Absolute cap on a transaction's gas limit from Amsterdam ([EIP-8037]).
///
/// [EIP-8037]: https://eips.ethereum.org/EIPS/eip-8037
// TODO: replace with revm's constant once
// <https://github.com/bluealloy/revm/issues/3929> is resolved.
pub const TX_MAX_TOTAL_GAS_LIMIT: u64 = u32::MAX as u64;

/// Returns the bound on a transaction's gas limit for the given hardfork.
pub fn max_transaction_gas_limit_for_hardfork<HardforkT: Into<EvmSpecId>>(
    hardfork: HardforkT,
) -> Option<u64> {
    let evm_spec_id = hardfork.into();
    if evm_spec_id >= EvmSpecId::AMSTERDAM {
        Some(TX_MAX_TOTAL_GAS_LIMIT)
    } else {
        edr_eip7825::transaction_gas_cap_for_hardfork(evm_spec_id)
    }
}

/// The gas bounds a transaction must satisfy to be included in a block.
///
/// Before Amsterdam `tx.gas` was a single quantity, and [EIP-7825] introduced
/// a cap on it. [EIP-8037] splits it: from Amsterdam `tx.gas` funds two
/// dimensions, execution gas and state gas, and the EIP-7825 cap is
/// repurposed to bound execution gas alone, while `tx.gas` as a whole gets a
/// new bound, `TX_MAX_TOTAL_GAS_LIMIT`. State gas has no bound of its own: its
/// reservoir is the difference between `tx.gas` and the execution bound, and
/// once it runs out state charges draw on the execution budget, so the two
/// bounds cover it transitively.
///
/// Before Amsterdam `tx.gas` represents a single thing, so both fields hold
/// the same value.
///
/// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
/// [EIP-8037]: https://eips.ethereum.org/EIPS/eip-8037
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransactionGasBounds {
    /// Bound on the gas a transaction may spend on execution. The protocol's
    /// value is the [EIP-7825] cap.
    ///
    /// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
    pub execution_gas: Option<u64>,
    /// Bound on the transaction's gas limit, i.e. on both dimensions together.
    pub total_transaction_gas: Option<u64>,
}

impl TransactionGasBounds {
    /// The protocol's bounds for the given hardfork.
    pub fn for_hardfork<HardforkT: Into<EvmSpecId>>(hardfork: HardforkT) -> Self {
        let evm_spec_id = hardfork.into();
        Self {
            execution_gas: edr_eip7825::transaction_gas_cap_for_hardfork(evm_spec_id),
            total_transaction_gas: max_transaction_gas_limit_for_hardfork(evm_spec_id),
        }
    }

    /// The protocol's bounds with the [EIP-7825] cap set to
    /// `execution_gas_cap`.
    ///
    /// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
    pub fn custom<HardforkT: Into<EvmSpecId>>(hardfork: HardforkT, execution_gas_cap: u64) -> Self {
        Self::with_execution_gas(hardfork, Some(execution_gas_cap))
    }

    /// The protocol's bounds without the [EIP-7825] cap.
    ///
    /// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
    pub fn disabled<HardforkT: Into<EvmSpecId>>(hardfork: HardforkT) -> Self {
        Self::with_execution_gas(hardfork, None)
    }

    fn with_execution_gas(hardfork: impl Into<EvmSpecId>, execution_gas: Option<u64>) -> Self {
        let evm_spec_id = hardfork.into();
        if evm_spec_id >= EvmSpecId::AMSTERDAM {
            Self {
                execution_gas,
                total_transaction_gas: max_transaction_gas_limit_for_hardfork(evm_spec_id),
            }
        } else {
            Self {
                execution_gas,
                total_transaction_gas: execution_gas,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use edr_eip7825::OSAKA_TRANSACTION_GAS_CAP;

    use super::*;

    const CUSTOM_CAP: u64 = 50_000;

    #[test]
    fn protocol_bounds_by_hardfork() {
        assert_eq!(
            TransactionGasBounds::for_hardfork(EvmSpecId::PRAGUE),
            TransactionGasBounds {
                execution_gas: None,
                total_transaction_gas: None,
            }
        );
        assert_eq!(
            TransactionGasBounds::for_hardfork(EvmSpecId::OSAKA),
            TransactionGasBounds {
                execution_gas: Some(OSAKA_TRANSACTION_GAS_CAP),
                total_transaction_gas: Some(OSAKA_TRANSACTION_GAS_CAP),
            }
        );
        assert_eq!(
            TransactionGasBounds::for_hardfork(EvmSpecId::AMSTERDAM),
            TransactionGasBounds {
                execution_gas: Some(OSAKA_TRANSACTION_GAS_CAP),
                total_transaction_gas: Some(TX_MAX_TOTAL_GAS_LIMIT),
            }
        );
    }

    #[test]
    fn custom_cap_bounds_gas_limit_before_amsterdam() {
        assert_eq!(
            TransactionGasBounds::custom(EvmSpecId::OSAKA, CUSTOM_CAP),
            TransactionGasBounds {
                execution_gas: Some(CUSTOM_CAP),
                total_transaction_gas: Some(CUSTOM_CAP),
            }
        );
    }

    #[test]
    fn custom_cap_keeps_protocol_gas_limit_bound_from_amsterdam() {
        assert_eq!(
            TransactionGasBounds::custom(EvmSpecId::AMSTERDAM, CUSTOM_CAP),
            TransactionGasBounds {
                execution_gas: Some(CUSTOM_CAP),
                total_transaction_gas: Some(TX_MAX_TOTAL_GAS_LIMIT),
            }
        );
    }

    #[test]
    fn disabled_cap_lifts_gas_limit_bound_before_amsterdam() {
        assert_eq!(
            TransactionGasBounds::disabled(EvmSpecId::OSAKA),
            TransactionGasBounds {
                execution_gas: None,
                total_transaction_gas: None,
            }
        );
    }

    #[test]
    fn disabled_cap_keeps_protocol_gas_limit_bound_from_amsterdam() {
        assert_eq!(
            TransactionGasBounds::disabled(EvmSpecId::AMSTERDAM),
            TransactionGasBounds {
                execution_gas: None,
                total_transaction_gas: Some(TX_MAX_TOTAL_GAS_LIMIT),
            }
        );
    }
}
