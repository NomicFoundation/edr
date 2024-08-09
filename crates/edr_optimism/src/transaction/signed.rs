/// The deposit transaction type
/// <https://specs.optimism.io/protocol/deposits.html#the-deposited-transaction-type>
mod deposit;

use std::sync::OnceLock;

use alloy_rlp::{Buf, RlpDecodable, RlpEncodable};
pub use edr_eth::transaction::signed::{Eip155, Eip1559, Eip2930, Eip4844, Legacy};
use edr_eth::{
    signature::Fakeable,
    transaction::{
        SignedTransaction, Transaction, TransactionMut, TransactionType, TransactionValidation,
        TxKind, INVALID_TX_TYPE_ERROR_MESSAGE,
    },
    Address, Bytes, B256, U256,
};
use revm::optimism::{OptimismInvalidTransaction, OptimismTransaction};

use super::Signed;

/// Deposit transaction.
///
/// For details, see <https://specs.optimism.io/protocol/deposits.html#the-deposited-transaction-type>.
#[derive(Clone, Debug, Eq, serde::Deserialize, RlpDecodable, RlpEncodable)]
#[serde(rename_all = "camelCase")]
pub struct Deposit {
    // The order of these fields determines encoding order.
    /// Hash that uniquely identifies the origin of the deposit.
    pub source_hash: B256,
    /// The address of the sender account.
    pub from: Address,
    /// The address of the recipient account, or the null (zero-length) address
    /// if the deposit transaction is a contract creation.
    pub to: TxKind,
    /// The ETH value to mint on L2.
    // #[serde(with = "edr_eth::serde::optional_u128")]
    pub mint: u128,
    ///  The ETH value to send to the recipient account.
    pub value: U256,
    /// The gas limit for the L2 transaction.
    #[serde(rename = "gas")]
    pub gas_limit: u64,
    /// Field indicating if this transaction is exempt from the L2 gas limit.
    pub is_system_tx: bool,
    #[serde(alias = "input")]
    /// The calldata
    pub data: Bytes,
    /// Cached transaction hash
    #[rlp(default)]
    #[rlp(skip)]
    #[serde(skip)]
    pub hash: OnceLock<B256>,
    /// Cached RLP-encoding
    #[rlp(default)]
    #[rlp(skip)]
    #[serde(skip)]
    pub rlp_encoding: OnceLock<Bytes>,
}

impl alloy_rlp::Decodable for Signed {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        match edr_eth::transaction::Signed::decode(buf) {
            Ok(transaction) => Ok(transaction.into()),
            Err(alloy_rlp::Error::Custom(INVALID_TX_TYPE_ERROR_MESSAGE)) => {
                let first = buf.first().ok_or(alloy_rlp::Error::InputTooShort)?;

                match *first {
                    Deposit::TYPE => {
                        buf.advance(1);

                        Ok(Signed::Deposit(Deposit::decode(buf)?))
                    }
                    _ => Err(alloy_rlp::Error::Custom(INVALID_TX_TYPE_ERROR_MESSAGE)),
                }
            }
            Err(error) => Err(error),
        }
    }
}

impl alloy_rlp::Encodable for Signed {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        let encoded = self.rlp_encoding();
        out.put_slice(encoded);
    }

    fn length(&self) -> usize {
        match self {
            Signed::PreEip155Legacy(tx) => tx.length(),
            Signed::PostEip155Legacy(tx) => tx.length(),
            Signed::Eip2930(tx) => tx.length() + 1,
            Signed::Eip1559(tx) => tx.length() + 1,
            Signed::Eip4844(tx) => tx.length() + 1,
            Signed::Deposit(tx) => tx.length() + 1,
        }
    }
}

impl Default for Signed {
    fn default() -> Self {
        // This implementation is necessary to be able to use `revm`'s builder pattern.
        Self::PreEip155Legacy(Legacy {
            nonce: 0,
            gas_price: U256::ZERO,
            gas_limit: u64::MAX,
            kind: TxKind::Call(Address::ZERO), // will do nothing
            value: U256::ZERO,
            input: Bytes::new(),
            signature: Fakeable::fake(Address::ZERO, Some(0)),
            hash: OnceLock::new(),
            rlp_encoding: OnceLock::new(),
        })
    }
}

impl From<edr_eth::transaction::Signed> for Signed {
    fn from(value: edr_eth::transaction::Signed) -> Self {
        match value {
            edr_eth::transaction::Signed::PreEip155Legacy(tx) => Self::PreEip155Legacy(tx),
            edr_eth::transaction::Signed::PostEip155Legacy(tx) => Self::PostEip155Legacy(tx),
            edr_eth::transaction::Signed::Eip2930(tx) => Self::Eip2930(tx),
            edr_eth::transaction::Signed::Eip1559(tx) => Self::Eip1559(tx),
            edr_eth::transaction::Signed::Eip4844(tx) => Self::Eip4844(tx),
        }
    }
}

impl OptimismTransaction for Signed {
    fn source_hash(&self) -> Option<&B256> {
        match self {
            Signed::Deposit(tx) => Some(&tx.source_hash),
            _ => None,
        }
    }

    fn mint(&self) -> Option<&u128> {
        match self {
            Signed::Deposit(tx) => Some(&tx.mint),
            _ => None,
        }
    }

    fn is_system_transaction(&self) -> Option<bool> {
        match self {
            Signed::Deposit(tx) => Some(tx.is_system_tx),
            _ => None,
        }
    }

    fn enveloped_tx(&self) -> Option<Bytes> {
        let enveloped = alloy_rlp::encode(self);
        Some(enveloped.into())
    }
}

impl SignedTransaction for Signed {
    fn effective_gas_price(&self, block_base_fee: U256) -> Option<U256> {
        match self {
            Signed::PreEip155Legacy(_)
            | Signed::PostEip155Legacy(_)
            | Signed::Eip2930(_)
            | Signed::Deposit(_) => None,
            Signed::Eip1559(tx) => Some(
                tx.max_fee_per_gas
                    .min(block_base_fee + tx.max_priority_fee_per_gas),
            ),
            Signed::Eip4844(tx) => Some(
                tx.max_fee_per_gas
                    .min(block_base_fee + tx.max_priority_fee_per_gas),
            ),
        }
    }

    fn max_fee_per_gas(&self) -> Option<U256> {
        match self {
            Signed::PreEip155Legacy(_)
            | Signed::PostEip155Legacy(_)
            | Signed::Eip2930(_)
            | Signed::Deposit(_) => None,
            Signed::Eip1559(tx) => Some(tx.max_fee_per_gas),
            Signed::Eip4844(tx) => Some(tx.max_fee_per_gas),
        }
    }

    fn rlp_encoding(&self) -> &Bytes {
        match self {
            Signed::PreEip155Legacy(tx) => tx.rlp_encoding(),
            Signed::PostEip155Legacy(tx) => tx.rlp_encoding(),
            Signed::Eip2930(tx) => tx.rlp_encoding(),
            Signed::Eip1559(tx) => tx.rlp_encoding(),
            Signed::Eip4844(tx) => tx.rlp_encoding(),
            Signed::Deposit(tx) => tx.rlp_encoding(),
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
            Signed::Deposit(tx) => tx.transaction_hash(),
        }
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
            Signed::Deposit(tx) => &tx.from,
        }
    }

    fn gas_limit(&self) -> u64 {
        match self {
            Signed::PreEip155Legacy(tx) => tx.gas_limit,
            Signed::PostEip155Legacy(tx) => tx.gas_limit,
            Signed::Eip2930(tx) => tx.gas_limit,
            Signed::Eip1559(tx) => tx.gas_limit,
            Signed::Eip4844(tx) => tx.gas_limit,
            Signed::Deposit(tx) => tx.gas_limit,
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
            Signed::Deposit(_) => &U256::ZERO,
        }
    }

    fn kind(&self) -> TxKind {
        match self {
            Signed::PreEip155Legacy(tx) => tx.kind,
            Signed::PostEip155Legacy(tx) => tx.kind,
            Signed::Eip2930(tx) => tx.kind,
            Signed::Eip1559(tx) => tx.kind,
            Signed::Eip4844(tx) => TxKind::Call(tx.to),
            Signed::Deposit(tx) => tx.to,
        }
    }

    fn value(&self) -> &U256 {
        match self {
            Signed::PreEip155Legacy(tx) => &tx.value,
            Signed::PostEip155Legacy(tx) => &tx.value,
            Signed::Eip2930(tx) => &tx.value,
            Signed::Eip1559(tx) => &tx.value,
            Signed::Eip4844(tx) => &tx.value,
            Signed::Deposit(tx) => &tx.value,
        }
    }

    fn data(&self) -> &Bytes {
        match self {
            Signed::PreEip155Legacy(tx) => &tx.input,
            Signed::PostEip155Legacy(tx) => &tx.input,
            Signed::Eip2930(tx) => &tx.input,
            Signed::Eip1559(tx) => &tx.input,
            Signed::Eip4844(tx) => &tx.input,
            Signed::Deposit(tx) => &tx.data,
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
            Signed::Deposit(_) => 0,
        }
    }

    fn chain_id(&self) -> Option<u64> {
        match self {
            Signed::PreEip155Legacy(_) | Signed::Deposit(_) => None,
            Signed::PostEip155Legacy(tx) => Some(tx.chain_id()),
            Signed::Eip2930(tx) => Some(tx.chain_id),
            Signed::Eip1559(tx) => Some(tx.chain_id),
            Signed::Eip4844(tx) => Some(tx.chain_id),
        }
    }

    fn access_list(&self) -> &[edr_eth::AccessListItem] {
        match self {
            Signed::PreEip155Legacy(_) | Signed::PostEip155Legacy(_) | Signed::Deposit(_) => &[],
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
            Signed::Deposit(_) => Some(&U256::ZERO),
        }
    }

    fn blob_hashes(&self) -> &[B256] {
        match self {
            Signed::PreEip155Legacy(_)
            | Signed::PostEip155Legacy(_)
            | Signed::Eip2930(_)
            | Signed::Eip1559(_)
            | Signed::Deposit(_) => &[],
            Signed::Eip4844(tx) => &tx.blob_hashes,
        }
    }

    fn max_fee_per_blob_gas(&self) -> Option<&U256> {
        match self {
            Signed::PreEip155Legacy(_)
            | Signed::PostEip155Legacy(_)
            | Signed::Eip2930(_)
            | Signed::Eip1559(_)
            | Signed::Deposit(_) => None,
            Signed::Eip4844(tx) => Some(&tx.max_fee_per_blob_gas),
        }
    }

    fn authorization_list(&self) -> Option<&edr_eth::env::AuthorizationList> {
        None
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
            Signed::Deposit(tx) => tx.gas_limit = gas_limit,
        }
    }
}

impl TransactionType for Signed {
    type Type = super::Type;

    fn transaction_type(&self) -> Self::Type {
        match self {
            Signed::PreEip155Legacy(_) | Signed::PostEip155Legacy(_) => super::Type::Legacy,
            Signed::Eip2930(_) => super::Type::Eip2930,
            Signed::Eip1559(_) => super::Type::Eip1559,
            Signed::Eip4844(_) => super::Type::Eip4844,
            Signed::Deposit(_) => super::Type::Deposit,
        }
    }
}

impl TransactionValidation for Signed {
    type ValidationError = OptimismInvalidTransaction;
}

#[cfg(test)]
mod tests {
    use alloy_rlp::Decodable as _;
    use edr_eth::hex;

    use super::*;

    #[test]
    fn signed_transaction_encoding_round_trip_deposit() -> anyhow::Result<()> {
        let bytes = Bytes::from_static(&hex!("7ef9015aa044bae9d41b8380d781187b426c6fe43df5fb2fb57bd4466ef6a701e1f01e015694deaddeaddeaddeaddeaddeaddeaddeaddead000194420000000000000000000000000000000000001580808408f0d18001b90104015d8eb900000000000000000000000000000000000000000000000000000000008057650000000000000000000000000000000000000000000000000000000063d96d10000000000000000000000000000000000000000000000000000000000009f35273d89754a1e0387b89520d989d3be9c37c1f32495a88faf1ea05c61121ab0d1900000000000000000000000000000000000000000000000000000000000000010000000000000000000000002d679b567db6187c0c8323fa982cfb88b74dbcc7000000000000000000000000000000000000000000000000000000000000083400000000000000000000000000000000000000000000000000000000000f4240"));

        let decoded = Signed::decode(&mut &bytes[..])?;
        let encoded = alloy_rlp::encode(&decoded);

        assert_eq!(encoded, bytes);

        Ok(())
    }
}
