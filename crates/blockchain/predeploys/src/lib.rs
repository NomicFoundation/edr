//! Accounts that hardforks require in state.

use edr_chain_spec::EvmSpecId;
use edr_eip2935::{history_storage_contract, HISTORY_STORAGE_ADDRESS};
use edr_eip4788::{beacon_roots_contract, BEACON_ROOTS_ADDRESS};
use edr_eip7997::{deterministic_factory_contract, DETERMINISTIC_FACTORY_ADDRESS};
use edr_primitives::Address;
use edr_state_api::account::AccountInfo;

/// Predeploy accounts, each paired with its constructor.
type Predeploys = &'static [(Address, fn() -> AccountInfo)];

/// Predeploys that a hardfork requires in state, keyed by the EVM spec that
/// introduces them.
const PREDEPLOY_ACTIVATIONS: &[(EvmSpecId, Predeploys)] = &[
    (
        EvmSpecId::CANCUN,
        &[(BEACON_ROOTS_ADDRESS, beacon_roots_contract)],
    ),
    (
        EvmSpecId::PRAGUE,
        &[(HISTORY_STORAGE_ADDRESS, history_storage_contract)],
    ),
    (
        EvmSpecId::AMSTERDAM,
        &[(
            DETERMINISTIC_FACTORY_ADDRESS,
            deterministic_factory_contract,
        )],
    ),
];

fn predeploys_where(activated: impl Fn(EvmSpecId) -> bool) -> Vec<(Address, AccountInfo)> {
    PREDEPLOY_ACTIVATIONS
        .iter()
        .filter(|(activation, _)| activated(*activation))
        .flat_map(|(_, predeploys)| {
            predeploys
                .iter()
                .map(|(address, constructor)| (*address, constructor()))
        })
        .collect()
}

/// Returns the predeploys that `hardfork` requires in state, i.e. those
/// introduced by it or by any earlier hardfork.
pub fn predeploys_for_hardfork(hardfork: EvmSpecId) -> Vec<(Address, AccountInfo)> {
    predeploys_where(|activation| activation <= hardfork)
}

/// Returns the predeploys introduced after `from`, up to and including `to`.
pub fn predeploys_activated_between(from: EvmSpecId, to: EvmSpecId) -> Vec<(Address, AccountInfo)> {
    predeploys_where(|activation| from < activation && activation <= to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addresses(predeploys: Vec<(Address, AccountInfo)>) -> Vec<Address> {
        predeploys.into_iter().map(|(address, _)| address).collect()
    }

    #[test]
    fn hardfork_requires_its_own_and_earlier_predeploys() {
        assert!(predeploys_for_hardfork(EvmSpecId::SHANGHAI).is_empty());
        assert_eq!(
            addresses(predeploys_for_hardfork(EvmSpecId::CANCUN)),
            vec![BEACON_ROOTS_ADDRESS]
        );
        assert_eq!(
            addresses(predeploys_for_hardfork(EvmSpecId::AMSTERDAM)),
            vec![
                BEACON_ROOTS_ADDRESS,
                HISTORY_STORAGE_ADDRESS,
                DETERMINISTIC_FACTORY_ADDRESS
            ]
        );
    }

    #[test]
    fn predeploys_span_every_hardfork_after_from_up_to_to() {
        assert_eq!(
            addresses(predeploys_activated_between(
                EvmSpecId::SHANGHAI,
                EvmSpecId::AMSTERDAM
            )),
            vec![
                BEACON_ROOTS_ADDRESS,
                HISTORY_STORAGE_ADDRESS,
                DETERMINISTIC_FACTORY_ADDRESS
            ]
        );
    }

    #[test]
    fn predeploys_exclude_hardforks_up_to_from() {
        assert_eq!(
            addresses(predeploys_activated_between(
                EvmSpecId::CANCUN,
                EvmSpecId::PRAGUE
            )),
            vec![HISTORY_STORAGE_ADDRESS]
        );
        assert_eq!(
            addresses(predeploys_activated_between(
                EvmSpecId::OSAKA,
                EvmSpecId::AMSTERDAM
            )),
            vec![DETERMINISTIC_FACTORY_ADDRESS]
        );
    }

    #[test]
    fn no_predeploys_when_to_does_not_exceed_from() {
        assert!(predeploys_activated_between(EvmSpecId::PRAGUE, EvmSpecId::PRAGUE).is_empty());
        assert!(predeploys_activated_between(EvmSpecId::AMSTERDAM, EvmSpecId::CANCUN).is_empty());
    }
}
