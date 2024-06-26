/// The deposited transaction type
/// <https://specs.optimism.io/protocol/deposits.html#the-deposited-transaction-type>
mod deposited;

use std::sync::OnceLock;

use alloy_rlp::RlpEncodable;
pub use edr_eth::transaction::signed::{Eip155, Eip1559, Eip2930, Eip4844, Legacy};
use edr_eth::{
    transaction::{SignedTransaction, Transaction, TransactionMut, TransactionValidation, TxKind},
    Address, Bytes, B256, U256,
};
use revm::optimism::InvalidOptimismTransaction;

use super::Signed;

/// Deposited transaction.
///
/// For details, see <https://specs.optimism.io/protocol/deposits.html#the-deposited-transaction-type>.
#[derive(Clone, Debug, Eq, serde::Deserialize, RlpEncodable)]
#[serde(rename_all = "camelCase")]
pub struct Deposited {
    /// Hash that uniquely identifies the origin of the deposit.
    pub source_hash: B256,
    /// The address of the sender account.
    pub from: Address,
    /// The address of the recipient account, or the null (zero-length) address
    /// if the deposited transaction is a contract creation.
    pub to: TxKind,
    /// The ETH value to mint on L2.
    #[serde(deserialize_with = "edr_eth::serde::optional_to_default")]
    pub mint: u128,
    ///  The ETH value to send to the recipient account.
    pub value: U256,
    /// The gas limit for the L2 transaction.
    #[serde(rename = "gas")]
    pub gas_limit: u64,
    /// Field indicating if this transaction is exempt from the L2 gas limit.
    #[serde(rename = "isSystemTx")]
    pub is_system_transaction: bool,
    #[serde(alias = "input")]
    /// The calldata
    pub data: Bytes,
    /// Cached transaction hash
    #[rlp(skip)]
    #[serde(skip)]
    pub hash: OnceLock<B256>,
}

impl SignedTransaction for Signed {
    fn effective_gas_price(&self, block_base_fee: U256) -> U256 {
        todo!()
    }

    fn max_fee_per_gas(&self) -> Option<U256> {
        match self {
            Signed::PreEip155Legacy(_)
            | Signed::PostEip155Legacy(_)
            | Signed::Eip2930(_)
            | Signed::Deposited(_) => None,
            Signed::Eip1559(tx) => Some(tx.max_fee_per_gas),
            Signed::Eip4844(tx) => Some(tx.max_fee_per_gas),
        }
    }

    fn total_blob_gas(&self) -> Option<u64> {
        match self {
            Signed::Eip4844(tx) => Some(tx.total_blob_gas()),
            _ => None,
        }
    }

    fn transaction_hash(&self) -> &B256 {
        match self {
            Signed::PreEip155Legacy(tx) => tx.transaction_hash(),
            Signed::PostEip155Legacy(tx) => tx.transaction_hash(),
            Signed::Eip2930(tx) => tx.transaction_hash(),
            Signed::Eip1559(tx) => tx.transaction_hash(),
            Signed::Eip4844(tx) => tx.transaction_hash(),
            Signed::Deposited(tx) => tx.transaction_hash(),
        }
    }

    fn transaction_type(&self) -> edr_eth::transaction::TransactionType {
        todo!()
    }
}

impl Transaction for Signed {
    fn caller(&self) -> &Address {
        match self {
            Signed::PreEip155Legacy(tx) => tx.caller(),
            Signed::PostEip155Legacy(tx) => tx.caller(),
            Signed::Eip2930(tx) => tx.caller(),
            Signed::Eip1559(tx) => tx.caller(),
            Signed::Eip4844(tx) => tx.caller(),
            Signed::Deposited(tx) => &tx.from,
        }
    }

    fn gas_limit(&self) -> u64 {
        match self {
            Signed::PreEip155Legacy(tx) => tx.gas_limit,
            Signed::PostEip155Legacy(tx) => tx.gas_limit,
            Signed::Eip2930(tx) => tx.gas_limit,
            Signed::Eip1559(tx) => tx.gas_limit,
            Signed::Eip4844(tx) => tx.gas_limit,
            Signed::Deposited(tx) => tx.gas_limit,
        }
    }

    fn gas_price(&self) -> &U256 {
        match self {
            Signed::PreEip155Legacy(tx) => &tx.gas_price,
            Signed::PostEip155Legacy(tx) => &tx.gas_price,
            Signed::Eip2930(tx) => &tx.gas_price,
            Signed::Eip1559(tx) => &tx.max_fee_per_gas,
            Signed::Eip4844(tx) => &tx.max_fee_per_gas,
            // No gas is refunded as ETH. (either by not refunding or utilizing the fact the
            // gas-price of the deposit is 0)
            Signed::Deposited(_) => &U256::ZERO,
        }
    }

    fn kind(&self) -> TxKind {
        match self {
            Signed::PreEip155Legacy(tx) => tx.kind,
            Signed::PostEip155Legacy(tx) => tx.kind,
            Signed::Eip2930(tx) => tx.kind,
            Signed::Eip1559(tx) => tx.kind,
            Signed::Eip4844(tx) => TxKind::Call(tx.to),
            Signed::Deposited(tx) => tx.to,
        }
    }

    fn value(&self) -> &U256 {
        match self {
            Signed::PreEip155Legacy(tx) => &tx.value,
            Signed::PostEip155Legacy(tx) => &tx.value,
            Signed::Eip2930(tx) => &tx.value,
            Signed::Eip1559(tx) => &tx.value,
            Signed::Eip4844(tx) => &tx.value,
            Signed::Deposited(tx) => &tx.value,
        }
    }

    fn data(&self) -> &Bytes {
        match self {
            Signed::PreEip155Legacy(tx) => &tx.input,
            Signed::PostEip155Legacy(tx) => &tx.input,
            Signed::Eip2930(tx) => &tx.input,
            Signed::Eip1559(tx) => &tx.input,
            Signed::Eip4844(tx) => &tx.input,
            Signed::Deposited(tx) => &tx.data,
        }
    }

    fn nonce(&self) -> u64 {
        match self {
            Signed::PreEip155Legacy(tx) => tx.nonce,
            Signed::PostEip155Legacy(tx) => tx.nonce,
            Signed::Eip2930(tx) => tx.nonce,
            Signed::Eip1559(tx) => tx.nonce,
            Signed::Eip4844(tx) => tx.nonce,
            // Before Regolith: the nonce is always 0
            // With Regolith: the nonce is set to the depositNonce attribute of the corresponding
            // transaction receipt.
            Signed::Deposited(_) => 0,
        }
    }

    fn chain_id(&self) -> Option<u64> {
        match self {
            Signed::PreEip155Legacy(_) | Signed::Deposited(_) => None,
            Signed::PostEip155Legacy(tx) => Some(tx.chain_id()),
            Signed::Eip2930(tx) => Some(tx.chain_id),
            Signed::Eip1559(tx) => Some(tx.chain_id),
            Signed::Eip4844(tx) => Some(tx.chain_id),
        }
    }

    fn access_list(&self) -> &[edr_eth::AccessListItem] {
        match self {
            Signed::PreEip155Legacy(_) | Signed::PostEip155Legacy(_) | Signed::Deposited(_) => &[],
            Signed::Eip2930(tx) => &tx.access_list.0,
            Signed::Eip1559(tx) => &tx.access_list.0,
            Signed::Eip4844(tx) => &tx.access_list.0,
        }
    }

    fn max_priority_fee_per_gas(&self) -> Option<&U256> {
        match self {
            Signed::PreEip155Legacy(_) | Signed::PostEip155Legacy(_) | Signed::Eip2930(_) => None,
            Signed::Eip1559(tx) => Some(&tx.max_priority_fee_per_gas),
            Signed::Eip4844(tx) => Some(&tx.max_priority_fee_per_gas),
            // No transaction priority fee is charged. No payment is made to the block
            // fee-recipient.
            Signed::Deposited(_) => Some(&U256::ZERO),
        }
    }

    fn blob_hashes(&self) -> &[B256] {
        match self {
            Signed::PreEip155Legacy(_)
            | Signed::PostEip155Legacy(_)
            | Signed::Eip2930(_)
            | Signed::Eip1559(_)
            | Signed::Deposited(_) => &[],
            Signed::Eip4844(tx) => &tx.blob_hashes,
        }
    }

    fn max_fee_per_blob_gas(&self) -> Option<&U256> {
        match self {
            Signed::PreEip155Legacy(_)
            | Signed::PostEip155Legacy(_)
            | Signed::Eip2930(_)
            | Signed::Eip1559(_)
            | Signed::Deposited(_) => None,
            Signed::Eip4844(tx) => Some(&tx.max_fee_per_blob_gas),
        }
    }
}

impl TransactionMut for Signed {
    fn set_gas_limit(&mut self, gas_limit: u64) {
        match self {
            Signed::PreEip155Legacy(tx) => tx.gas_limit = gas_limit,
            Signed::PostEip155Legacy(tx) => tx.gas_limit = gas_limit,
            Signed::Eip2930(tx) => tx.gas_limit = gas_limit,
            Signed::Eip1559(tx) => tx.gas_limit = gas_limit,
            Signed::Eip4844(tx) => tx.gas_limit = gas_limit,
            Signed::Deposited(tx) => tx.gas_limit = gas_limit,
        }
    }
}

impl TransactionValidation for Signed {
    type ValidationError = InvalidOptimismTransaction;
}
