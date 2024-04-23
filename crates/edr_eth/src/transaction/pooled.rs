mod eip4844;

pub use self::eip4844::Eip4844PooledTransaction;
use super::{
    Eip1559SignedTransaction, Eip155SignedTransaction, Eip2930SignedTransaction,
    LegacySignedTransaction, SignedTransaction,
};
use crate::{transaction::INVALID_TX_TYPE_ERROR_MESSAGE, utils::enveloped};

pub type LegacyPooledTransaction = LegacySignedTransaction;
pub type Eip155PooledTransaction = Eip155SignedTransaction;
pub type Eip2930PooledTransaction = Eip2930SignedTransaction;
pub type Eip1559PooledTransaction = Eip1559SignedTransaction;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PooledTransaction {
    /// Legacy transaction
    PreEip155Legacy(LegacyPooledTransaction),
    /// EIP-155 transaction
    PostEip155Legacy(Eip155PooledTransaction),
    /// EIP-2930 transaction
    Eip2930(Eip2930PooledTransaction),
    /// EIP-1559 transaction
    Eip1559(Eip1559PooledTransaction),
    /// EIP-4844 transaction
    Eip4844(Eip4844PooledTransaction),
}

impl PooledTransaction {
    /// Converts the pooled transaction into a signed transaction.
    pub fn into_payload(self) -> SignedTransaction {
        match self {
            PooledTransaction::PreEip155Legacy(tx) => SignedTransaction::PreEip155Legacy(tx),
            PooledTransaction::PostEip155Legacy(tx) => SignedTransaction::PostEip155Legacy(tx),
            PooledTransaction::Eip2930(tx) => SignedTransaction::Eip2930(tx),
            PooledTransaction::Eip1559(tx) => SignedTransaction::Eip1559(tx),
            PooledTransaction::Eip4844(tx) => SignedTransaction::Eip4844(tx.into_payload()),
        }
    }
}

impl alloy_rlp::Decodable for PooledTransaction {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        use alloy_rlp::Buf;

        fn is_list(byte: u8) -> bool {
            byte >= 0xc0
        }

        let first = buf.first().ok_or(alloy_rlp::Error::InputTooShort)?;

        match *first {
            0x01 => {
                buf.advance(1);

                Ok(PooledTransaction::Eip2930(
                    Eip2930PooledTransaction::decode(buf)?,
                ))
            }
            0x02 => {
                buf.advance(1);

                Ok(PooledTransaction::Eip1559(
                    Eip1559PooledTransaction::decode(buf)?,
                ))
            }
            0x03 => {
                buf.advance(1);

                Ok(PooledTransaction::Eip4844(
                    Eip4844PooledTransaction::decode(buf)?,
                ))
            }
            byte if is_list(byte) => {
                let tx = LegacyPooledTransaction::decode(buf)?;
                if tx.signature.v >= 35 {
                    Ok(PooledTransaction::PostEip155Legacy(tx.into()))
                } else {
                    Ok(PooledTransaction::PreEip155Legacy(tx))
                }
            }
            _ => Err(alloy_rlp::Error::Custom(INVALID_TX_TYPE_ERROR_MESSAGE)),
        }
    }
}

impl alloy_rlp::Encodable for PooledTransaction {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        match self {
            PooledTransaction::PreEip155Legacy(tx) => tx.encode(out),
            PooledTransaction::PostEip155Legacy(tx) => tx.encode(out),
            PooledTransaction::Eip2930(tx) => enveloped(1, tx, out),
            PooledTransaction::Eip1559(tx) => enveloped(2, tx, out),
            PooledTransaction::Eip4844(tx) => enveloped(3, tx, out),
        }
    }

    fn length(&self) -> usize {
        match self {
            PooledTransaction::PreEip155Legacy(tx) => tx.length(),
            PooledTransaction::PostEip155Legacy(tx) => tx.length(),
            PooledTransaction::Eip2930(tx) => tx.length() + 1,
            PooledTransaction::Eip1559(tx) => tx.length() + 1,
            PooledTransaction::Eip4844(tx) => tx.length() + 1,
        }
    }
}

impl From<LegacyPooledTransaction> for PooledTransaction {
    fn from(value: LegacyPooledTransaction) -> Self {
        PooledTransaction::PreEip155Legacy(value)
    }
}

impl From<Eip155PooledTransaction> for PooledTransaction {
    fn from(value: Eip155PooledTransaction) -> Self {
        PooledTransaction::PostEip155Legacy(value)
    }
}

impl From<Eip2930PooledTransaction> for PooledTransaction {
    fn from(value: Eip2930PooledTransaction) -> Self {
        PooledTransaction::Eip2930(value)
    }
}

impl From<Eip1559PooledTransaction> for PooledTransaction {
    fn from(value: Eip1559PooledTransaction) -> Self {
        PooledTransaction::Eip1559(value)
    }
}

impl From<Eip4844PooledTransaction> for PooledTransaction {
    fn from(value: Eip4844PooledTransaction) -> Self {
        PooledTransaction::Eip4844(value)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use alloy_rlp::Decodable;
    use revm_primitives::EnvKzgSettings;

    use super::*;
    use crate::{
        signature::Signature,
        transaction::{Eip4844SignedTransaction, TransactionKind},
        Address, Bytes, U256,
    };

    macro_rules! impl_test_pooled_transaction_encoding_round_trip {
        ($(
            $name:ident => $transaction:expr,
        )+) => {
            $(
                paste::item! {
                    #[test]
                    fn [<pooled_transaction_encoding_round_trip_ $name>]() -> anyhow::Result<()> {
                        let transaction = $transaction;

                        let encoded = alloy_rlp::encode(&transaction);
                        let decoded = PooledTransaction::decode(&mut encoded.as_slice()).unwrap();

                        assert_eq!(decoded, transaction);

                        Ok(())
                    }
                }
            )+
        };
    }

    impl_test_pooled_transaction_encoding_round_trip! {
        pre_eip155 => PooledTransaction::PreEip155Legacy(LegacyPooledTransaction {
            nonce: 0,
            gas_price: U256::from(1),
            gas_limit: 2,
            kind: TransactionKind::Call(Address::default()),
            value: U256::from(3),
            input: Bytes::from(vec![1, 2]),
            signature: Signature {
                r: U256::default(),
                s: U256::default(),
                v: 1,
            },
            hash: OnceLock::new(),
            is_fake: false
        }),
        post_eip155 => PooledTransaction::PostEip155Legacy(Eip155PooledTransaction {
            nonce: 0,
            gas_price: U256::from(1),
            gas_limit: 2,
            kind: TransactionKind::Create,
            value: U256::from(3),
            input: Bytes::from(vec![1, 2]),
            signature: Signature {
                r: U256::default(),
                s: U256::default(),
                v: 37,
            },
            hash: OnceLock::new(),
            is_fake: false
        }),
        eip2930 => PooledTransaction::Eip2930(Eip2930PooledTransaction {
            chain_id: 1,
            nonce: 0,
            gas_price: U256::from(1),
            gas_limit: 2,
            kind: TransactionKind::Call(Address::random()),
            value: U256::from(3),
            input: Bytes::from(vec![1, 2]),
            odd_y_parity: true,
            r: U256::default(),
            s: U256::default(),
            access_list: vec![].into(),
            hash: OnceLock::new(),
            is_fake: false
        }),
        eip1559 => PooledTransaction::Eip1559(Eip1559PooledTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(1),
            max_fee_per_gas: U256::from(2),
            gas_limit: 3,
            kind: TransactionKind::Create,
            value: U256::from(4),
            input: Bytes::from(vec![1, 2]),
            access_list: vec![].into(),
            odd_y_parity: true,
            r: U256::default(),
            s: U256::default(),
            hash: OnceLock::new(),
            is_fake: false
        }),
        eip4844 => PooledTransaction::Eip4844(Eip4844PooledTransaction::new(Eip4844SignedTransaction {
            chain_id: 1337,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(1_000_000_000),
            max_fee_per_gas: U256::from(1_000_000_000),
            max_fee_per_blob_gas: U256::from(1),
            gas_limit: 1_000_000,
            to: Address::ZERO,
            value: U256::from(4),
            input: Bytes::from(vec![1, 2]),
            access_list: vec![].into(),
            blob_hashes: vec![],
            odd_y_parity: true,
            r: U256::default(),
            s: U256::default(),
            hash: OnceLock::new(),
            is_fake: false
        }, vec![], vec![], vec![], EnvKzgSettings::Default.get())?),
    }
}
