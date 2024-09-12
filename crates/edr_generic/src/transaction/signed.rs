use edr_eth::transaction::{self, ExecutableTransaction, TransactionType};
use revm_primitives::{
    AccessListItem, Address, AuthorizationList, Bytes, TransactionValidation, TxKind, B256, U256,
};

/// The type of transaction.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Type {
    #[default]
    /// Legacy transaction
    Legacy = transaction::signed::Legacy::TYPE,
    /// EIP-2930 transaction
    Eip2930 = transaction::signed::Eip2930::TYPE,
    /// EIP-1559 transaction
    Eip1559 = transaction::signed::Eip1559::TYPE,
    /// EIP-4844 transaction
    Eip4844 = transaction::signed::Eip4844::TYPE,
    /// Unrecognized transaction type.
    Unrecognized(u8),
}

impl From<Type> for u8 {
    fn from(t: Type) -> u8 {
        match t {
            Type::Legacy => transaction::signed::Legacy::TYPE,
            Type::Eip2930 => transaction::signed::Eip2930::TYPE,
            Type::Eip1559 => transaction::signed::Eip1559::TYPE,
            Type::Eip4844 => transaction::signed::Eip4844::TYPE,
            Type::Unrecognized(t) => t,
        }
    }
}

impl From<u8> for Type {
    fn from(t: u8) -> Self {
        match t {
            transaction::signed::Legacy::TYPE => Self::Legacy,
            transaction::signed::Eip2930::TYPE => Self::Eip2930,
            transaction::signed::Eip1559::TYPE => Self::Eip1559,
            transaction::signed::Eip4844::TYPE => Self::Eip4844,
            t => Self::Unrecognized(t),
        }
    }
}

impl From<edr_eth::transaction::Type> for Type {
    fn from(t: edr_eth::transaction::Type) -> Self {
        match t {
            edr_eth::transaction::Type::Legacy => Self::Legacy,
            edr_eth::transaction::Type::Eip2930 => Self::Eip2930,
            edr_eth::transaction::Type::Eip1559 => Self::Eip1559,
            edr_eth::transaction::Type::Eip4844 => Self::Eip4844,
        }
    }
}

impl transaction::IsEip4844 for Type {
    fn is_eip4844(&self) -> bool {
        matches!(self, Type::Eip4844)
    }
}

impl transaction::IsLegacy for Type {
    fn is_legacy(&self) -> bool {
        matches!(self, Type::Legacy)
    }
}

/// A regular [`Signed`](edr_eth::transaction::Signed) transaction that falls
/// back to post-EIP 155 legacy transactions for unknown transaction types
/// when converting from an RPC request.
// NOTE: This is a newtype only because we need to use a different
// `TryFrom<TransactionWithSignature>` impl that treats unknown transaction
// types different.
#[repr(transparent)]
#[derive(Clone, Debug, Default, PartialEq, Eq, alloy_rlp::RlpEncodable)]
pub struct SignedWithFallbackToPostEip155(pub transaction::Signed);

impl From<transaction::Signed> for SignedWithFallbackToPostEip155 {
    fn from(value: transaction::Signed) -> Self {
        Self(value)
    }
}

impl From<edr_eth::transaction::pooled::PooledTransaction> for SignedWithFallbackToPostEip155 {
    fn from(value: edr_eth::transaction::pooled::PooledTransaction) -> Self {
        edr_eth::transaction::Signed::from(value).into()
    }
}

impl TransactionValidation for SignedWithFallbackToPostEip155 {
    type ValidationError = <transaction::Signed as TransactionValidation>::ValidationError;
}

impl revm_primitives::Transaction for SignedWithFallbackToPostEip155 {
    fn caller(&self) -> &Address {
        self.0.caller()
    }

    fn gas_limit(&self) -> u64 {
        self.0.gas_limit()
    }

    fn gas_price(&self) -> &U256 {
        self.0.gas_price()
    }

    fn kind(&self) -> TxKind {
        self.0.kind()
    }

    fn value(&self) -> &U256 {
        self.0.value()
    }

    fn data(&self) -> &Bytes {
        self.0.data()
    }

    fn nonce(&self) -> u64 {
        self.0.nonce()
    }

    fn chain_id(&self) -> Option<u64> {
        self.0.chain_id()
    }

    fn access_list(&self) -> &[AccessListItem] {
        self.0.access_list()
    }

    fn max_priority_fee_per_gas(&self) -> Option<&U256> {
        self.0.max_priority_fee_per_gas()
    }

    fn blob_hashes(&self) -> &[B256] {
        self.0.blob_hashes()
    }

    fn max_fee_per_blob_gas(&self) -> Option<&U256> {
        self.0.max_fee_per_blob_gas()
    }

    fn authorization_list(&self) -> Option<&AuthorizationList> {
        self.0.authorization_list()
    }
}

impl ExecutableTransaction for SignedWithFallbackToPostEip155 {
    fn effective_gas_price(&self, block_base_fee: U256) -> Option<U256> {
        self.0.effective_gas_price(block_base_fee)
    }

    fn max_fee_per_gas(&self) -> Option<&U256> {
        self.0.max_fee_per_gas()
    }

    fn rlp_encoding(&self) -> &Bytes {
        self.0.rlp_encoding()
    }

    fn total_blob_gas(&self) -> Option<u64> {
        self.0.total_blob_gas()
    }

    fn transaction_hash(&self) -> &B256 {
        self.0.transaction_hash()
    }
}

impl TransactionType for SignedWithFallbackToPostEip155 {
    type Type = crate::transaction::Type;

    fn transaction_type(&self) -> Self::Type {
        self.0.transaction_type().into()
    }
}

impl transaction::HasAccessList for SignedWithFallbackToPostEip155 {
    fn has_access_list(&self) -> bool {
        self.0.has_access_list()
    }
}

impl transaction::IsEip155 for SignedWithFallbackToPostEip155 {
    fn is_eip155(&self) -> bool {
        self.0.is_eip155()
    }
}

impl transaction::IsEip4844 for SignedWithFallbackToPostEip155 {
    fn is_eip4844(&self) -> bool {
        self.0.is_eip4844()
    }
}

impl transaction::IsLegacy for SignedWithFallbackToPostEip155 {
    fn is_legacy(&self) -> bool {
        self.0.is_legacy()
    }
}
