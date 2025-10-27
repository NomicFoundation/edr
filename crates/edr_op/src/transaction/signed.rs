/// The deposit transaction type
/// <https://specs.optimism.io/protocol/deposits.html#the-deposited-transaction-type>
mod deposit;

use std::sync::OnceLock;

use alloy_rlp::{Buf, RlpDecodable, RlpEncodable};
use edr_chain_spec::{ExecutableTransaction, TransactionValidation};
use edr_primitives::{Address, Bytes, B256, U256};
use edr_signer::{FakeableSignature, Signature};
pub use edr_transaction::signed::{Eip155, Eip1559, Eip2930, Eip4844, Eip7702, Legacy};
use edr_transaction::{
    impl_revm_transaction_trait, IsEip4844, IsLegacy, IsSupported, MaybeSignedTransaction,
    TransactionMut, TransactionType, TxKind, INVALID_TX_TYPE_ERROR_MESSAGE,
};

use super::OpSignedTransaction;
use crate::transaction::{InvalidTransaction, OpTxTrait};

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
    #[serde(with = "alloy_serde::quantity")]
    pub mint: u128,
    ///  The ETH value to send to the recipient account.
    pub value: U256,
    /// The gas limit for the L2 transaction.
    #[serde(rename = "gas", with = "alloy_serde::quantity")]
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

impl alloy_rlp::Decodable for OpSignedTransaction {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        match edr_chain_l1::L1SignedTransaction::decode(buf) {
            Ok(transaction) => Ok(transaction.into()),
            Err(alloy_rlp::Error::Custom(INVALID_TX_TYPE_ERROR_MESSAGE)) => {
                let first = buf.first().ok_or(alloy_rlp::Error::InputTooShort)?;

                match *first {
                    Deposit::TYPE => {
                        buf.advance(1);

                        Ok(OpSignedTransaction::Deposit(Deposit::decode(buf)?))
                    }
                    _ => Err(alloy_rlp::Error::Custom(INVALID_TX_TYPE_ERROR_MESSAGE)),
                }
            }
            Err(error) => Err(error),
        }
    }
}

impl alloy_rlp::Encodable for OpSignedTransaction {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        let encoded = self.rlp_encoding();
        out.put_slice(encoded);
    }

    fn length(&self) -> usize {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.length(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.length(),
            OpSignedTransaction::Eip2930(tx) => tx.length() + 1,
            OpSignedTransaction::Eip1559(tx) => tx.length() + 1,
            OpSignedTransaction::Eip4844(tx) => tx.length() + 1,
            OpSignedTransaction::Eip7702(tx) => tx.length() + 1,
            OpSignedTransaction::Deposit(tx) => tx.length() + 1,
        }
    }
}

impl Default for OpSignedTransaction {
    fn default() -> Self {
        // This implementation is necessary to be able to use `revm`'s builder pattern.
        Self::PreEip155Legacy(Legacy {
            nonce: 0,
            gas_price: 0,
            gas_limit: u64::MAX,
            kind: TxKind::Call(Address::ZERO), // will do nothing
            value: U256::ZERO,
            input: Bytes::new(),
            signature: FakeableSignature::fake(Address::ZERO, Some(0)),
            hash: OnceLock::new(),
            rlp_encoding: OnceLock::new(),
        })
    }
}

impl From<edr_chain_l1::L1SignedTransaction> for OpSignedTransaction {
    fn from(value: edr_chain_l1::L1SignedTransaction) -> Self {
        match value {
            edr_chain_l1::L1SignedTransaction::PreEip155Legacy(tx) => Self::PreEip155Legacy(tx),
            edr_chain_l1::L1SignedTransaction::PostEip155Legacy(tx) => Self::PostEip155Legacy(tx),
            edr_chain_l1::L1SignedTransaction::Eip2930(tx) => Self::Eip2930(tx),
            edr_chain_l1::L1SignedTransaction::Eip1559(tx) => Self::Eip1559(tx),
            edr_chain_l1::L1SignedTransaction::Eip4844(tx) => Self::Eip4844(tx),
            edr_chain_l1::L1SignedTransaction::Eip7702(tx) => Self::Eip7702(tx),
        }
    }
}

impl From<Deposit> for OpSignedTransaction {
    fn from(transaction: Deposit) -> Self {
        Self::Deposit(transaction)
    }
}

impl From<Eip155> for OpSignedTransaction {
    fn from(transaction: Eip155) -> Self {
        Self::PostEip155Legacy(transaction)
    }
}

impl From<Eip2930> for OpSignedTransaction {
    fn from(transaction: Eip2930) -> Self {
        Self::Eip2930(transaction)
    }
}

impl From<Eip1559> for OpSignedTransaction {
    fn from(transaction: Eip1559) -> Self {
        Self::Eip1559(transaction)
    }
}

impl From<Eip4844> for OpSignedTransaction {
    fn from(transaction: Eip4844) -> Self {
        Self::Eip4844(transaction)
    }
}

impl From<Eip7702> for OpSignedTransaction {
    fn from(transaction: Eip7702) -> Self {
        Self::Eip7702(transaction)
    }
}

impl From<Legacy> for OpSignedTransaction {
    fn from(transaction: Legacy) -> Self {
        Self::PreEip155Legacy(transaction)
    }
}

impl IsEip4844 for OpSignedTransaction {
    fn is_eip4844(&self) -> bool {
        matches!(self, OpSignedTransaction::Eip4844(_))
    }
}

impl IsSupported for OpSignedTransaction {
    fn is_supported_transaction(&self) -> bool {
        true
    }
}

impl IsLegacy for OpSignedTransaction {
    fn is_legacy(&self) -> bool {
        matches!(
            self,
            OpSignedTransaction::PreEip155Legacy(_) | OpSignedTransaction::PostEip155Legacy(_)
        )
    }
}

impl MaybeSignedTransaction for OpSignedTransaction {
    fn maybe_signature(&self) -> Option<&dyn Signature> {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => Some(&tx.signature),
            OpSignedTransaction::PostEip155Legacy(tx) => Some(&tx.signature),
            OpSignedTransaction::Eip2930(tx) => Some(&tx.signature),
            OpSignedTransaction::Eip1559(tx) => Some(&tx.signature),
            OpSignedTransaction::Eip4844(tx) => Some(&tx.signature),
            OpSignedTransaction::Eip7702(tx) => Some(&tx.signature),
            OpSignedTransaction::Deposit(_) => None,
        }
    }
}

impl OpTxTrait for OpSignedTransaction {
    fn enveloped_tx(&self) -> Option<&Bytes> {
        Some(self.rlp_encoding())
    }

    fn source_hash(&self) -> Option<B256> {
        match self {
            OpSignedTransaction::Deposit(tx) => Some(tx.source_hash),
            _ => None,
        }
    }

    fn mint(&self) -> Option<u128> {
        match self {
            OpSignedTransaction::Deposit(tx) => Some(tx.mint),
            _ => None,
        }
    }

    fn is_system_transaction(&self) -> bool {
        match self {
            OpSignedTransaction::Deposit(tx) => tx.is_system_tx,
            _ => false,
        }
    }
}

impl ExecutableTransaction for OpSignedTransaction {
    fn caller(&self) -> &Address {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.caller(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.caller(),
            OpSignedTransaction::Eip2930(tx) => tx.caller(),
            OpSignedTransaction::Eip1559(tx) => tx.caller(),
            OpSignedTransaction::Eip4844(tx) => tx.caller(),
            OpSignedTransaction::Eip7702(tx) => tx.caller(),
            OpSignedTransaction::Deposit(tx) => tx.caller(),
        }
    }

    fn gas_limit(&self) -> u64 {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.gas_limit(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.gas_limit(),
            OpSignedTransaction::Eip2930(tx) => tx.gas_limit(),
            OpSignedTransaction::Eip1559(tx) => tx.gas_limit(),
            OpSignedTransaction::Eip4844(tx) => tx.gas_limit(),
            OpSignedTransaction::Eip7702(tx) => tx.gas_limit(),
            OpSignedTransaction::Deposit(tx) => tx.gas_limit(),
        }
    }

    fn gas_price(&self) -> &u128 {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.gas_price(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.gas_price(),
            OpSignedTransaction::Eip2930(tx) => tx.gas_price(),
            OpSignedTransaction::Eip1559(tx) => tx.gas_price(),
            OpSignedTransaction::Eip4844(tx) => tx.gas_price(),
            OpSignedTransaction::Eip7702(tx) => tx.gas_price(),
            OpSignedTransaction::Deposit(tx) => tx.gas_price(),
        }
    }

    fn kind(&self) -> TxKind {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.kind(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.kind(),
            OpSignedTransaction::Eip2930(tx) => tx.kind(),
            OpSignedTransaction::Eip1559(tx) => tx.kind(),
            OpSignedTransaction::Eip4844(tx) => tx.kind(),
            OpSignedTransaction::Eip7702(tx) => tx.kind(),
            OpSignedTransaction::Deposit(tx) => tx.kind(),
        }
    }

    fn value(&self) -> &U256 {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.value(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.value(),
            OpSignedTransaction::Eip2930(tx) => tx.value(),
            OpSignedTransaction::Eip1559(tx) => tx.value(),
            OpSignedTransaction::Eip4844(tx) => tx.value(),
            OpSignedTransaction::Eip7702(tx) => tx.value(),
            OpSignedTransaction::Deposit(tx) => tx.value(),
        }
    }

    fn data(&self) -> &Bytes {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.data(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.data(),
            OpSignedTransaction::Eip2930(tx) => tx.data(),
            OpSignedTransaction::Eip1559(tx) => tx.data(),
            OpSignedTransaction::Eip4844(tx) => tx.data(),
            OpSignedTransaction::Eip7702(tx) => tx.data(),
            OpSignedTransaction::Deposit(tx) => tx.data(),
        }
    }

    fn nonce(&self) -> u64 {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.nonce(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.nonce(),
            OpSignedTransaction::Eip2930(tx) => tx.nonce(),
            OpSignedTransaction::Eip1559(tx) => tx.nonce(),
            OpSignedTransaction::Eip4844(tx) => tx.nonce(),
            OpSignedTransaction::Eip7702(tx) => tx.nonce(),
            OpSignedTransaction::Deposit(tx) => tx.nonce(),
        }
    }

    fn chain_id(&self) -> Option<u64> {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.chain_id(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.chain_id(),
            OpSignedTransaction::Eip2930(tx) => tx.chain_id(),
            OpSignedTransaction::Eip1559(tx) => tx.chain_id(),
            OpSignedTransaction::Eip4844(tx) => tx.chain_id(),
            OpSignedTransaction::Eip7702(tx) => tx.chain_id(),
            OpSignedTransaction::Deposit(tx) => tx.chain_id(),
        }
    }

    fn access_list(&self) -> Option<&[edr_eip2930::AccessListItem]> {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.access_list(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.access_list(),
            OpSignedTransaction::Eip2930(tx) => tx.access_list(),
            OpSignedTransaction::Eip1559(tx) => tx.access_list(),
            OpSignedTransaction::Eip4844(tx) => tx.access_list(),
            OpSignedTransaction::Eip7702(tx) => tx.access_list(),
            OpSignedTransaction::Deposit(tx) => tx.access_list(),
        }
    }

    fn effective_gas_price(&self, block_base_fee: u128) -> Option<u128> {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.effective_gas_price(block_base_fee),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.effective_gas_price(block_base_fee),
            OpSignedTransaction::Eip2930(tx) => tx.effective_gas_price(block_base_fee),
            OpSignedTransaction::Eip1559(tx) => tx.effective_gas_price(block_base_fee),
            OpSignedTransaction::Eip4844(tx) => tx.effective_gas_price(block_base_fee),
            OpSignedTransaction::Eip7702(tx) => tx.effective_gas_price(block_base_fee),
            OpSignedTransaction::Deposit(tx) => tx.effective_gas_price(block_base_fee),
        }
    }

    fn max_fee_per_gas(&self) -> Option<&u128> {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.max_fee_per_gas(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.max_fee_per_gas(),
            OpSignedTransaction::Eip2930(tx) => tx.max_fee_per_gas(),
            OpSignedTransaction::Eip1559(tx) => tx.max_fee_per_gas(),
            OpSignedTransaction::Eip4844(tx) => tx.max_fee_per_gas(),
            OpSignedTransaction::Eip7702(tx) => tx.max_fee_per_gas(),
            OpSignedTransaction::Deposit(tx) => tx.max_fee_per_gas(),
        }
    }

    fn max_priority_fee_per_gas(&self) -> Option<&u128> {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.max_priority_fee_per_gas(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.max_priority_fee_per_gas(),
            OpSignedTransaction::Eip2930(tx) => tx.max_priority_fee_per_gas(),
            OpSignedTransaction::Eip1559(tx) => tx.max_priority_fee_per_gas(),
            OpSignedTransaction::Eip4844(tx) => tx.max_priority_fee_per_gas(),
            OpSignedTransaction::Eip7702(tx) => tx.max_priority_fee_per_gas(),
            OpSignedTransaction::Deposit(tx) => tx.max_priority_fee_per_gas(),
        }
    }

    fn blob_hashes(&self) -> &[B256] {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.blob_hashes(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.blob_hashes(),
            OpSignedTransaction::Eip2930(tx) => tx.blob_hashes(),
            OpSignedTransaction::Eip1559(tx) => tx.blob_hashes(),
            OpSignedTransaction::Eip4844(tx) => tx.blob_hashes(),
            OpSignedTransaction::Eip7702(tx) => tx.blob_hashes(),
            OpSignedTransaction::Deposit(tx) => tx.blob_hashes(),
        }
    }

    fn max_fee_per_blob_gas(&self) -> Option<&u128> {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.max_fee_per_blob_gas(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.max_fee_per_blob_gas(),
            OpSignedTransaction::Eip2930(tx) => tx.max_fee_per_blob_gas(),
            OpSignedTransaction::Eip1559(tx) => tx.max_fee_per_blob_gas(),
            OpSignedTransaction::Eip4844(tx) => tx.max_fee_per_blob_gas(),
            OpSignedTransaction::Eip7702(tx) => tx.max_fee_per_blob_gas(),
            OpSignedTransaction::Deposit(tx) => tx.max_fee_per_blob_gas(),
        }
    }

    fn total_blob_gas(&self) -> Option<u64> {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.total_blob_gas(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.total_blob_gas(),
            OpSignedTransaction::Eip2930(tx) => tx.total_blob_gas(),
            OpSignedTransaction::Eip1559(tx) => tx.total_blob_gas(),
            OpSignedTransaction::Eip4844(tx) => tx.total_blob_gas(),
            OpSignedTransaction::Eip7702(tx) => tx.total_blob_gas(),
            OpSignedTransaction::Deposit(tx) => tx.total_blob_gas(),
        }
    }

    fn authorization_list(&self) -> Option<&[edr_eip7702::SignedAuthorization]> {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.authorization_list(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.authorization_list(),
            OpSignedTransaction::Eip2930(tx) => tx.authorization_list(),
            OpSignedTransaction::Eip1559(tx) => tx.authorization_list(),
            OpSignedTransaction::Eip4844(tx) => tx.authorization_list(),
            OpSignedTransaction::Eip7702(tx) => tx.authorization_list(),
            OpSignedTransaction::Deposit(tx) => tx.authorization_list(),
        }
    }

    fn rlp_encoding(&self) -> &Bytes {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.rlp_encoding(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.rlp_encoding(),
            OpSignedTransaction::Eip2930(tx) => tx.rlp_encoding(),
            OpSignedTransaction::Eip1559(tx) => tx.rlp_encoding(),
            OpSignedTransaction::Eip4844(tx) => tx.rlp_encoding(),
            OpSignedTransaction::Eip7702(tx) => tx.rlp_encoding(),
            OpSignedTransaction::Deposit(tx) => tx.rlp_encoding(),
        }
    }

    fn transaction_hash(&self) -> &B256 {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.transaction_hash(),
            OpSignedTransaction::PostEip155Legacy(tx) => tx.transaction_hash(),
            OpSignedTransaction::Eip2930(tx) => tx.transaction_hash(),
            OpSignedTransaction::Eip1559(tx) => tx.transaction_hash(),
            OpSignedTransaction::Eip4844(tx) => tx.transaction_hash(),
            OpSignedTransaction::Eip7702(tx) => tx.transaction_hash(),
            OpSignedTransaction::Deposit(tx) => tx.transaction_hash(),
        }
    }
}

impl TransactionMut for OpSignedTransaction {
    fn set_gas_limit(&mut self, gas_limit: u64) {
        match self {
            OpSignedTransaction::PreEip155Legacy(tx) => tx.gas_limit = gas_limit,
            OpSignedTransaction::PostEip155Legacy(tx) => tx.gas_limit = gas_limit,
            OpSignedTransaction::Eip2930(tx) => tx.gas_limit = gas_limit,
            OpSignedTransaction::Eip1559(tx) => tx.gas_limit = gas_limit,
            OpSignedTransaction::Eip4844(tx) => tx.gas_limit = gas_limit,
            OpSignedTransaction::Eip7702(tx) => tx.gas_limit = gas_limit,
            OpSignedTransaction::Deposit(tx) => tx.gas_limit = gas_limit,
        }
    }
}

impl TransactionType for OpSignedTransaction {
    type Type = super::OpTransactionType;

    fn transaction_type(&self) -> Self::Type {
        match self {
            OpSignedTransaction::PreEip155Legacy(_) | OpSignedTransaction::PostEip155Legacy(_) => {
                super::OpTransactionType::Legacy
            }
            OpSignedTransaction::Eip2930(_) => super::OpTransactionType::Eip2930,
            OpSignedTransaction::Eip1559(_) => super::OpTransactionType::Eip1559,
            OpSignedTransaction::Eip4844(_) => super::OpTransactionType::Eip4844,
            OpSignedTransaction::Eip7702(_) => super::OpTransactionType::Eip7702,
            OpSignedTransaction::Deposit(_) => super::OpTransactionType::Deposit,
        }
    }
}

impl TransactionValidation for OpSignedTransaction {
    type ValidationError = InvalidTransaction;
}

impl_revm_transaction_trait!(OpSignedTransaction);

#[cfg(test)]
mod tests {
    use alloy_rlp::Decodable as _;
    use edr_primitives::hex;

    use super::*;

    #[test]
    fn signed_transaction_encoding_round_trip_deposit() -> anyhow::Result<()> {
        let bytes = Bytes::from_static(&hex!(
            "7ef9015aa044bae9d41b8380d781187b426c6fe43df5fb2fb57bd4466ef6a701e1f01e015694deaddeaddeaddeaddeaddeaddeaddeaddead000194420000000000000000000000000000000000001580808408f0d18001b90104015d8eb900000000000000000000000000000000000000000000000000000000008057650000000000000000000000000000000000000000000000000000000063d96d10000000000000000000000000000000000000000000000000000000000009f35273d89754a1e0387b89520d989d3be9c37c1f32495a88faf1ea05c61121ab0d1900000000000000000000000000000000000000000000000000000000000000010000000000000000000000002d679b567db6187c0c8323fa982cfb88b74dbcc7000000000000000000000000000000000000000000000000000000000000083400000000000000000000000000000000000000000000000000000000000f4240"
        ));

        let decoded = OpSignedTransaction::decode(&mut &bytes[..])?;
        let encoded = alloy_rlp::encode(&decoded);

        assert_eq!(encoded, bytes);

        Ok(())
    }
}
