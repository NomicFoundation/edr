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
