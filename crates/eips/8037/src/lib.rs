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

/// Hardfork-specific per-transaction gas bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransactionGasBounds {
    /// The [EIP-7825] cap. Before Amsterdam all of a transaction's gas is
    /// execution gas, so it bounds the gas limit too; from Amsterdam it bounds
    /// execution gas only.
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

    /// The protocol's bounds with the [EIP-7825] cap replaced by
    /// `execution_gas_cap`. Before Amsterdam it bounds the gas limit too; from
    /// Amsterdam the gas limit keeps the protocol's bound.
    ///
    /// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
    pub fn custom<HardforkT: Into<EvmSpecId>>(hardfork: HardforkT, execution_gas_cap: u64) -> Self {
        let evm_spec_id = hardfork.into();
        let max_transaction_gas_limit = if evm_spec_id >= EvmSpecId::AMSTERDAM {
            max_transaction_gas_limit_for_hardfork(evm_spec_id)
        } else {
            Some(execution_gas_cap)
        };
        Self {
            execution_gas: Some(execution_gas_cap),
            total_transaction_gas: max_transaction_gas_limit,
        }
    }

    /// No per-transaction gas bounds.
    pub const fn disabled() -> Self {
        Self {
            execution_gas: None,
            total_transaction_gas: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy_eips::eip7825::MAX_TX_GAS_LIMIT_OSAKA;

    use super::*;

    #[test]
    fn protocol_bounds_by_hardfork() {
        assert_eq!(
            TransactionGasBounds::for_hardfork(EvmSpecId::PRAGUE),
            TransactionGasBounds::disabled()
        );
        assert_eq!(
            TransactionGasBounds::for_hardfork(EvmSpecId::OSAKA),
            TransactionGasBounds {
                execution_gas: Some(MAX_TX_GAS_LIMIT_OSAKA),
                total_transaction_gas: Some(MAX_TX_GAS_LIMIT_OSAKA),
            }
        );
        assert_eq!(
            TransactionGasBounds::for_hardfork(EvmSpecId::AMSTERDAM),
            TransactionGasBounds {
                execution_gas: Some(MAX_TX_GAS_LIMIT_OSAKA),
                total_transaction_gas: Some(TX_MAX_TOTAL_GAS_LIMIT),
            }
        );
    }

    #[test]
    fn custom_cap_bounds_the_gas_limit_only_before_amsterdam() {
        const CUSTOM_CAP: u64 = 50_000;

        assert_eq!(
            TransactionGasBounds::custom(EvmSpecId::OSAKA, CUSTOM_CAP),
            TransactionGasBounds {
                execution_gas: Some(CUSTOM_CAP),
                total_transaction_gas: Some(CUSTOM_CAP),
            }
        );
        assert_eq!(
            TransactionGasBounds::custom(EvmSpecId::AMSTERDAM, CUSTOM_CAP),
            TransactionGasBounds {
                execution_gas: Some(CUSTOM_CAP),
                total_transaction_gas: Some(TX_MAX_TOTAL_GAS_LIMIT),
            }
        );
    }
}
