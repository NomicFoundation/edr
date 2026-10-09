#![warn(missing_docs)]

//! Types related to EIP-2780.

use edr_chain_spec::EvmSpecId;
use edr_transaction::Transaction;
pub use revm_context_interface::cfg::gas_params::Eip2780TxInfo;

/// Returns the transaction inputs that EIP-2780 adds to the intrinsic gas
/// calculation, or `None` on hardforks that predate EIP-2780.
pub fn transaction_intrinsic_gas_info_for_hardfork<HardforkT: Into<EvmSpecId>>(
    transaction: &impl Transaction,
    hardfork: HardforkT,
) -> Option<Eip2780TxInfo> {
    (hardfork.into() >= EvmSpecId::AMSTERDAM).then(|| Eip2780TxInfo {
        value: transaction.value(),
        is_self_transfer: transaction.kind().to() == Some(&transaction.caller()),
    })
}

#[cfg(test)]
mod tests {
    use edr_chain_l1::L1SignedTransaction;
    use edr_transaction::{
        primitives::{Address, Bytes, U256},
        TxKind,
    };

    use super::*;

    fn transaction(caller: Address, kind: TxKind, value: U256) -> L1SignedTransaction {
        edr_chain_l1::request::Eip155 {
            nonce: 0,
            gas_price: 0,
            gas_limit: 21_000,
            kind,
            value,
            input: Bytes::new(),
            chain_id: 1,
        }
        .fake_sign(caller)
        .into()
    }

    mod transaction_intrinsic_gas_info_for_hardfork {
        use super::*;

        #[test]
        fn is_none_before_amsterdam() {
            let caller = Address::random();
            let transaction = transaction(caller, TxKind::Call(Address::random()), U256::from(1));

            assert_eq!(
                transaction_intrinsic_gas_info_for_hardfork(&transaction, EvmSpecId::OSAKA),
                None
            );
        }

        #[test]
        fn reports_value_and_no_self_transfer_for_a_call_to_another_account() {
            let caller = Address::random();
            let value = U256::from(7);
            let transaction = transaction(caller, TxKind::Call(Address::random()), value);

            assert_eq!(
                transaction_intrinsic_gas_info_for_hardfork(&transaction, EvmSpecId::AMSTERDAM),
                Some(Eip2780TxInfo {
                    value,
                    is_self_transfer: false,
                })
            );
        }

        #[test]
        fn reports_a_self_transfer_for_a_call_to_the_sender() {
            let caller = Address::random();
            let value = U256::from(7);
            let transaction = transaction(caller, TxKind::Call(caller), value);

            assert_eq!(
                transaction_intrinsic_gas_info_for_hardfork(&transaction, EvmSpecId::AMSTERDAM),
                Some(Eip2780TxInfo {
                    value,
                    is_self_transfer: true,
                })
            );
        }

        #[test]
        fn never_reports_a_create_as_a_self_transfer() {
            let caller = Address::random();
            let transaction = transaction(caller, TxKind::Create, U256::ZERO);

            assert_eq!(
                transaction_intrinsic_gas_info_for_hardfork(&transaction, EvmSpecId::AMSTERDAM),
                Some(Eip2780TxInfo {
                    value: U256::ZERO,
                    is_self_transfer: false,
                })
            );
        }
    }
}
