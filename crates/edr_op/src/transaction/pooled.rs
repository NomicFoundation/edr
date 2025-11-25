use edr_chain_spec::ExecutableTransaction;
use edr_primitives::{Address, Bytes, TxKind, B256, U256};
use edr_provider::spec::HardforkValidationData;
pub use edr_transaction::pooled::{Eip155, Eip1559, Eip2930, Eip4844, Eip7702, Legacy};
use edr_transaction::{
    pooled::eip4844::Blob, signed::PreOrPostEip155, IsEip155, INVALID_TX_TYPE_ERROR_MESSAGE,
};

use crate::transaction::signed::OpSignedTransaction;

/// An OP deposit pooled transaction.
pub type Deposit = super::signed::Deposit;

/// An OP pooled transaction, used to communicate between node pools.
pub enum OpPooledTransaction {
    /// Legacy transaction before EIP-155
    PreEip155Legacy(Legacy),
    /// Legacy transaction after EIP-155
    PostEip155Legacy(Eip155),
    /// EIP-2930 transaction
    Eip2930(Eip2930),
    /// EIP-1559 transaction
    Eip1559(Eip1559),
    /// EIP-4844 transaction
    Eip4844(Eip4844),
    /// EIP-7702 transaction
    Eip7702(Eip7702),
    /// OP deposit transaction
    Deposit(Deposit),
}

impl OpPooledTransaction {
    /// Converts the pooled transaction into a signed transaction.
    pub fn into_payload(self) -> OpSignedTransaction {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => OpSignedTransaction::PreEip155Legacy(tx),
            OpPooledTransaction::PostEip155Legacy(tx) => OpSignedTransaction::PostEip155Legacy(tx),
            OpPooledTransaction::Eip2930(tx) => OpSignedTransaction::Eip2930(tx),
            OpPooledTransaction::Eip1559(tx) => OpSignedTransaction::Eip1559(tx),
            OpPooledTransaction::Eip4844(tx) => OpSignedTransaction::Eip4844(tx.into_payload()),
            OpPooledTransaction::Eip7702(tx) => OpSignedTransaction::Eip7702(tx),
            OpPooledTransaction::Deposit(tx) => OpSignedTransaction::Deposit(tx),
        }
    }
}

impl alloy_rlp::Decodable for OpPooledTransaction {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        use alloy_rlp::Buf;

        fn is_list(byte: u8) -> bool {
            byte >= 0xc0
        }

        let first = buf.first().ok_or(alloy_rlp::Error::InputTooShort)?;

        match *first {
            Eip2930::TYPE => {
                buf.advance(1);

                Ok(OpPooledTransaction::Eip2930(Eip2930::decode(buf)?))
            }
            Eip1559::TYPE => {
                buf.advance(1);

                Ok(OpPooledTransaction::Eip1559(Eip1559::decode(buf)?))
            }
            Eip4844::TYPE => {
                buf.advance(1);

                Ok(OpPooledTransaction::Eip4844(Eip4844::decode(buf)?))
            }
            Deposit::TYPE => {
                buf.advance(1);

                Ok(OpPooledTransaction::Deposit(Deposit::decode(buf)?))
            }
            byte if is_list(byte) => {
                let transaction = PreOrPostEip155::decode(buf)?;
                Ok(transaction.into())
            }
            _ => Err(alloy_rlp::Error::Custom(INVALID_TX_TYPE_ERROR_MESSAGE)),
        }
    }
}

impl alloy_rlp::Encodable for OpPooledTransaction {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        let encoded = self.rlp_encoding();
        out.put_slice(encoded);
    }

    fn length(&self) -> usize {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.length(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.length(),
            OpPooledTransaction::Eip2930(tx) => tx.length() + 1,
            OpPooledTransaction::Eip1559(tx) => tx.length() + 1,
            OpPooledTransaction::Eip4844(tx) => tx.length() + 1,
            OpPooledTransaction::Eip7702(tx) => tx.length() + 1,
            OpPooledTransaction::Deposit(tx) => tx.length() + 1,
        }
    }
}

impl From<OpPooledTransaction> for OpSignedTransaction {
    fn from(value: OpPooledTransaction) -> Self {
        value.into_payload()
    }
}

impl From<PreOrPostEip155> for OpPooledTransaction {
    fn from(value: PreOrPostEip155) -> Self {
        match value {
            PreOrPostEip155::Pre(tx) => Self::PreEip155Legacy(tx),
            PreOrPostEip155::Post(tx) => Self::PostEip155Legacy(tx),
        }
    }
}

impl HardforkValidationData for OpPooledTransaction {
    fn to(&self) -> Option<&Address> {
        Some(self.caller())
    }

    fn gas_price(&self) -> Option<&u128> {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => Some(&tx.gas_price),
            OpPooledTransaction::PostEip155Legacy(tx) => Some(&tx.gas_price),
            OpPooledTransaction::Eip2930(tx) => Some(&tx.gas_price),
            OpPooledTransaction::Eip1559(_)
            | OpPooledTransaction::Eip4844(_)
            | OpPooledTransaction::Eip7702(_)
            | OpPooledTransaction::Deposit(_) => None,
        }
    }

    fn max_fee_per_gas(&self) -> Option<&u128> {
        ExecutableTransaction::max_fee_per_gas(self)
    }

    fn max_priority_fee_per_gas(&self) -> Option<&u128> {
        ExecutableTransaction::max_priority_fee_per_gas(self)
    }

    fn access_list(&self) -> Option<&Vec<edr_eip2930::AccessListItem>> {
        match self {
            OpPooledTransaction::PreEip155Legacy(_)
            | OpPooledTransaction::PostEip155Legacy(_)
            | OpPooledTransaction::Deposit(_) => None,
            OpPooledTransaction::Eip2930(tx) => Some(tx.access_list.0.as_ref()),
            OpPooledTransaction::Eip1559(tx) => Some(tx.access_list.0.as_ref()),
            OpPooledTransaction::Eip4844(tx) => Some(&tx.payload().access_list),
            OpPooledTransaction::Eip7702(tx) => Some(tx.access_list.0.as_ref()),
        }
    }

    fn blobs(&self) -> Option<&Vec<Blob>> {
        match self {
            OpPooledTransaction::Eip4844(tx) => Some(tx.blobs_ref()),
            _ => None,
        }
    }

    fn blob_hashes(&self) -> Option<&Vec<B256>> {
        match self {
            OpPooledTransaction::Eip4844(tx) => Some(&tx.payload().blob_hashes),
            _ => None,
        }
    }

    fn authorization_list(&self) -> Option<&Vec<edr_eip7702::SignedAuthorization>> {
        match self {
            OpPooledTransaction::Eip7702(tx) => Some(&tx.authorization_list),
            _ => None,
        }
    }
}

impl IsEip155 for OpPooledTransaction {
    fn is_eip155(&self) -> bool {
        matches!(self, OpPooledTransaction::PostEip155Legacy(_))
    }
}

impl ExecutableTransaction for OpPooledTransaction {
    fn caller(&self) -> &Address {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.caller(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.caller(),
            OpPooledTransaction::Eip2930(tx) => tx.caller(),
            OpPooledTransaction::Eip1559(tx) => tx.caller(),
            OpPooledTransaction::Eip4844(tx) => tx.caller(),
            OpPooledTransaction::Eip7702(tx) => tx.caller(),
            OpPooledTransaction::Deposit(tx) => tx.caller(),
        }
    }

    fn gas_limit(&self) -> u64 {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.gas_limit(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.gas_limit(),
            OpPooledTransaction::Eip2930(tx) => tx.gas_limit(),
            OpPooledTransaction::Eip1559(tx) => tx.gas_limit(),
            OpPooledTransaction::Eip4844(tx) => tx.gas_limit(),
            OpPooledTransaction::Eip7702(tx) => tx.gas_limit(),
            OpPooledTransaction::Deposit(tx) => tx.gas_limit(),
        }
    }

    fn gas_price(&self) -> &u128 {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.gas_price(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.gas_price(),
            OpPooledTransaction::Eip2930(tx) => tx.gas_price(),
            OpPooledTransaction::Eip1559(tx) => tx.gas_price(),
            OpPooledTransaction::Eip4844(tx) => tx.gas_price(),
            OpPooledTransaction::Eip7702(tx) => tx.gas_price(),
            OpPooledTransaction::Deposit(tx) => tx.gas_price(),
        }
    }

    fn kind(&self) -> TxKind {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.kind(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.kind(),
            OpPooledTransaction::Eip2930(tx) => tx.kind(),
            OpPooledTransaction::Eip1559(tx) => tx.kind(),
            OpPooledTransaction::Eip4844(tx) => tx.kind(),
            OpPooledTransaction::Eip7702(tx) => tx.kind(),
            OpPooledTransaction::Deposit(tx) => tx.kind(),
        }
    }

    fn value(&self) -> &U256 {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.value(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.value(),
            OpPooledTransaction::Eip2930(tx) => tx.value(),
            OpPooledTransaction::Eip1559(tx) => tx.value(),
            OpPooledTransaction::Eip4844(tx) => tx.value(),
            OpPooledTransaction::Eip7702(tx) => tx.value(),
            OpPooledTransaction::Deposit(tx) => tx.value(),
        }
    }

    fn data(&self) -> &Bytes {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.data(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.data(),
            OpPooledTransaction::Eip2930(tx) => tx.data(),
            OpPooledTransaction::Eip1559(tx) => tx.data(),
            OpPooledTransaction::Eip4844(tx) => tx.data(),
            OpPooledTransaction::Eip7702(tx) => tx.data(),
            OpPooledTransaction::Deposit(tx) => tx.data(),
        }
    }

    fn nonce(&self) -> u64 {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.nonce(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.nonce(),
            OpPooledTransaction::Eip2930(tx) => tx.nonce(),
            OpPooledTransaction::Eip1559(tx) => tx.nonce(),
            OpPooledTransaction::Eip4844(tx) => tx.nonce(),
            OpPooledTransaction::Eip7702(tx) => tx.nonce(),
            OpPooledTransaction::Deposit(tx) => tx.nonce(),
        }
    }

    fn chain_id(&self) -> Option<u64> {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.chain_id(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.chain_id(),
            OpPooledTransaction::Eip2930(tx) => tx.chain_id(),
            OpPooledTransaction::Eip1559(tx) => tx.chain_id(),
            OpPooledTransaction::Eip4844(tx) => tx.chain_id(),
            OpPooledTransaction::Eip7702(tx) => tx.chain_id(),
            OpPooledTransaction::Deposit(tx) => tx.chain_id(),
        }
    }

    fn access_list(&self) -> Option<&[edr_eip2930::AccessListItem]> {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.access_list(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.access_list(),
            OpPooledTransaction::Eip2930(tx) => tx.access_list(),
            OpPooledTransaction::Eip1559(tx) => tx.access_list(),
            OpPooledTransaction::Eip4844(tx) => tx.access_list(),
            OpPooledTransaction::Eip7702(tx) => tx.access_list(),
            OpPooledTransaction::Deposit(tx) => tx.access_list(),
        }
    }

    fn effective_gas_price(&self, block_base_fee: u128) -> Option<u128> {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.effective_gas_price(block_base_fee),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.effective_gas_price(block_base_fee),
            OpPooledTransaction::Eip2930(tx) => tx.effective_gas_price(block_base_fee),
            OpPooledTransaction::Eip1559(tx) => tx.effective_gas_price(block_base_fee),
            OpPooledTransaction::Eip4844(tx) => tx.effective_gas_price(block_base_fee),
            OpPooledTransaction::Eip7702(tx) => tx.effective_gas_price(block_base_fee),
            OpPooledTransaction::Deposit(tx) => tx.effective_gas_price(block_base_fee),
        }
    }

    fn max_fee_per_gas(&self) -> Option<&u128> {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.max_fee_per_gas(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.max_fee_per_gas(),
            OpPooledTransaction::Eip2930(tx) => tx.max_fee_per_gas(),
            OpPooledTransaction::Eip1559(tx) => tx.max_fee_per_gas(),
            OpPooledTransaction::Eip4844(tx) => tx.max_fee_per_gas(),
            OpPooledTransaction::Eip7702(tx) => tx.max_fee_per_gas(),
            OpPooledTransaction::Deposit(tx) => tx.max_fee_per_gas(),
        }
    }

    fn max_priority_fee_per_gas(&self) -> Option<&u128> {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.max_priority_fee_per_gas(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.max_priority_fee_per_gas(),
            OpPooledTransaction::Eip2930(tx) => tx.max_priority_fee_per_gas(),
            OpPooledTransaction::Eip1559(tx) => tx.max_priority_fee_per_gas(),
            OpPooledTransaction::Eip4844(tx) => tx.max_priority_fee_per_gas(),
            OpPooledTransaction::Eip7702(tx) => tx.max_priority_fee_per_gas(),
            OpPooledTransaction::Deposit(tx) => tx.max_priority_fee_per_gas(),
        }
    }

    fn blob_hashes(&self) -> &[B256] {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.blob_hashes(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.blob_hashes(),
            OpPooledTransaction::Eip2930(tx) => tx.blob_hashes(),
            OpPooledTransaction::Eip1559(tx) => tx.blob_hashes(),
            OpPooledTransaction::Eip4844(tx) => tx.blob_hashes(),
            OpPooledTransaction::Eip7702(tx) => tx.blob_hashes(),
            OpPooledTransaction::Deposit(tx) => tx.blob_hashes(),
        }
    }

    fn max_fee_per_blob_gas(&self) -> Option<&u128> {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.max_fee_per_blob_gas(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.max_fee_per_blob_gas(),
            OpPooledTransaction::Eip2930(tx) => tx.max_fee_per_blob_gas(),
            OpPooledTransaction::Eip1559(tx) => tx.max_fee_per_blob_gas(),
            OpPooledTransaction::Eip4844(tx) => tx.max_fee_per_blob_gas(),
            OpPooledTransaction::Eip7702(tx) => tx.max_fee_per_blob_gas(),
            OpPooledTransaction::Deposit(tx) => tx.max_fee_per_blob_gas(),
        }
    }

    fn total_blob_gas(&self) -> Option<u64> {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.total_blob_gas(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.total_blob_gas(),
            OpPooledTransaction::Eip2930(tx) => tx.total_blob_gas(),
            OpPooledTransaction::Eip1559(tx) => tx.total_blob_gas(),
            OpPooledTransaction::Eip4844(tx) => tx.total_blob_gas(),
            OpPooledTransaction::Eip7702(tx) => tx.total_blob_gas(),
            OpPooledTransaction::Deposit(tx) => tx.total_blob_gas(),
        }
    }

    fn authorization_list(&self) -> Option<&[edr_eip7702::SignedAuthorization]> {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.authorization_list(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.authorization_list(),
            OpPooledTransaction::Eip2930(tx) => tx.authorization_list(),
            OpPooledTransaction::Eip1559(tx) => tx.authorization_list(),
            OpPooledTransaction::Eip4844(tx) => tx.authorization_list(),
            OpPooledTransaction::Eip7702(tx) => tx.authorization_list(),
            OpPooledTransaction::Deposit(tx) => tx.authorization_list(),
        }
    }

    fn rlp_encoding(&self) -> &Bytes {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.rlp_encoding(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.rlp_encoding(),
            OpPooledTransaction::Eip2930(tx) => tx.rlp_encoding(),
            OpPooledTransaction::Eip1559(tx) => tx.rlp_encoding(),
            OpPooledTransaction::Eip4844(tx) => tx.rlp_encoding(),
            OpPooledTransaction::Eip7702(tx) => tx.rlp_encoding(),
            OpPooledTransaction::Deposit(tx) => tx.rlp_encoding(),
        }
    }

    fn transaction_hash(&self) -> &B256 {
        match self {
            OpPooledTransaction::PreEip155Legacy(tx) => tx.transaction_hash(),
            OpPooledTransaction::PostEip155Legacy(tx) => tx.transaction_hash(),
            OpPooledTransaction::Eip2930(tx) => tx.transaction_hash(),
            OpPooledTransaction::Eip1559(tx) => tx.transaction_hash(),
            OpPooledTransaction::Eip4844(tx) => tx.transaction_hash(),
            OpPooledTransaction::Eip7702(tx) => tx.transaction_hash(),
            OpPooledTransaction::Deposit(tx) => tx.transaction_hash(),
        }
    }
}
