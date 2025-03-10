mod eip4844;

pub use self::eip4844::Eip4844;
use super::Signed;
use crate::{
    transaction::{signed::PreOrPostEip155, INVALID_TX_TYPE_ERROR_MESSAGE},
    utils::enveloped,
};

pub type LegacyPooledTransaction = super::signed::Legacy;
pub type Eip155PooledTransaction = super::signed::Eip155;
pub type Eip2930PooledTransaction = super::signed::Eip2930;
pub type Eip1559PooledTransaction = super::signed::Eip1559;
pub type Eip7702 = super::signed::Eip7702;

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
    Eip4844(Eip4844),
    /// EIP-7702 transaction
    Eip7702(Eip7702),
}

impl PooledTransaction {
    /// Returns the blobs of the EIP-4844 transaction, if any.
    pub fn blobs(&self) -> Option<&[c_kzg::Blob]> {
        match self {
            PooledTransaction::Eip4844(tx) => Some(tx.blobs()),
            _ => None,
        }
    }

    /// Returns the commitments of the EIP-4844 transaction, if any.
    pub fn commitments(&self) -> Option<&[c_kzg::Bytes48]> {
        match self {
            PooledTransaction::Eip4844(tx) => Some(tx.commitments()),
            _ => None,
        }
    }

    /// Converts the pooled transaction into a signed transaction.
    pub fn into_payload(self) -> Signed {
        match self {
            PooledTransaction::PreEip155Legacy(tx) => Signed::PreEip155Legacy(tx),
            PooledTransaction::PostEip155Legacy(tx) => Signed::PostEip155Legacy(tx),
            PooledTransaction::Eip2930(tx) => Signed::Eip2930(tx),
            PooledTransaction::Eip1559(tx) => Signed::Eip1559(tx),
            PooledTransaction::Eip4844(tx) => Signed::Eip4844(tx.into_payload()),
            PooledTransaction::Eip7702(tx) => Signed::Eip7702(tx),
        }
    }

    /// Returns the proofs of the EIP-4844 transaction, if any.
    pub fn proofs(&self) -> Option<&[c_kzg::Bytes48]> {
        match self {
            PooledTransaction::Eip4844(tx) => Some(tx.proofs()),
            _ => None,
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

                Ok(PooledTransaction::Eip4844(Eip4844::decode(buf)?))
            }
            0x04 => {
                buf.advance(1);

                Ok(PooledTransaction::Eip7702(Eip7702::decode(buf)?))
            }
            byte if is_list(byte) => {
                let transaction = PreOrPostEip155::decode(buf)?;
                Ok(transaction.into())
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
            PooledTransaction::Eip7702(tx) => enveloped(4, tx, out),
        }
    }

    fn length(&self) -> usize {
        match self {
            PooledTransaction::PreEip155Legacy(tx) => tx.length(),
            PooledTransaction::PostEip155Legacy(tx) => tx.length(),
            PooledTransaction::Eip2930(tx) => tx.length() + 1,
            PooledTransaction::Eip1559(tx) => tx.length() + 1,
            PooledTransaction::Eip4844(tx) => tx.length() + 1,
            PooledTransaction::Eip7702(tx) => tx.length() + 1,
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

impl From<Eip4844> for PooledTransaction {
    fn from(value: Eip4844) -> Self {
        PooledTransaction::Eip4844(value)
    }
}

impl From<Eip7702> for PooledTransaction {
    fn from(value: Eip7702) -> Self {
        PooledTransaction::Eip7702(value)
    }
}

impl From<PreOrPostEip155> for PooledTransaction {
    fn from(value: PreOrPostEip155) -> Self {
        match value {
            PreOrPostEip155::Pre(tx) => Self::PreEip155Legacy(tx),
            PreOrPostEip155::Post(tx) => Self::PostEip155Legacy(tx),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::OnceLock};

    use alloy_rlp::Decodable;
    use c_kzg::BYTES_PER_BLOB;
    use revm_primitives::EnvKzgSettings;

    use super::*;
    use crate::{
        address,
        eips::eip7702,
        signature::{self, SignatureWithYParity, SignatureWithYParityArgs},
        transaction::{self, TxKind},
        Address, Bytes, B256, U256,
    };

    fn fake_eip4844_blob() -> c_kzg::Blob {
        const BLOB_VALUE: &[u8] = b"hello world";

        // The blob starts 0, followed by `hello world`, then 0x80, and is padded with
        // zeroes.
        let mut bytes = vec![0x0u8];
        bytes.append(&mut BLOB_VALUE.to_vec());
        bytes.push(alloy_rlp::EMPTY_STRING_CODE);

        bytes.resize(BYTES_PER_BLOB, 0);

        c_kzg::Blob::from_bytes(bytes.as_slice()).expect("Invalid blob")
    }

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
            kind: TxKind::Call(Address::default()),
            value: U256::from(3),
            input: Bytes::from(vec![1, 2]),
            // SAFETY: Signature and caller address have been precomputed based on
            // `crate::edr_eth::transaction::signed::impl_test_signed_transaction_encoding_round_trip!`
            signature: unsafe { signature::Fakeable::with_address_unchecked(
                signature::SignatureWithRecoveryId {
                    r: U256::from_str("0xf0407adecc60467f3293582a9e1d726db5bc6b64f230bfb6ff04f23a1bbfe8dc")?,
                    s: U256::from_str("0x2f68623b42c3b302b8b96035c30ca58c566fdfdc3421ddb4f41d61b485e1401b")?,
                    v: 27,
                },
                Address::from_str("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266")?,
            )},
            hash: OnceLock::new(),
        }),
        post_eip155 => PooledTransaction::PostEip155Legacy(Eip155PooledTransaction {
            nonce: 0,
            gas_price: U256::from(1),
            gas_limit: 2,
            kind: TxKind::Create,
            value: U256::from(3),
            input: Bytes::from(vec![1, 2]),
            // SAFETY: Signature and caller address have been precomputed based on
            // `crate::edr_eth::transaction::signed::impl_test_signed_transaction_encoding_round_trip!`
            signature: unsafe { signature::Fakeable::with_address_unchecked(
                signature::SignatureWithRecoveryId {
                    r: U256::from_str("0xed3a859fce13d142bba6051a91f934947f71c5f8ce8e3fe5bc7a845365309b90")?,
                    s: U256::from_str("0x1deb3bbf3fff7fba96e853ff1a19eabb117ef93b7704176893e4e9fff0e04576")?,
                    v: 2709,
                },
                Address::from_str("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266")?,
            )},
            hash: OnceLock::new(),
        }),
        eip2930 => PooledTransaction::Eip2930(Eip2930PooledTransaction {
            chain_id: 1,
            nonce: 0,
            gas_price: U256::from(1),
            gas_limit: 2,
            kind: TxKind::Call(Address::default()),
            value: U256::from(3),
            input: Bytes::from(vec![1, 2]),
            access_list: vec![].into(),
            // SAFETY: Signature and caller address have been precomputed based on
            // `crate::edr_eth::transaction::signed::impl_test_signed_transaction_encoding_round_trip!`
            signature: unsafe { signature::Fakeable::with_address_unchecked(
                SignatureWithYParity::new(
                    SignatureWithYParityArgs {
                        r: U256::from_str("0xa8d41ec812e66a7d80a1478f053cb8b627abb36191f53c2f7a153b4e4f90564d")?,
                        s: U256::from_str("0x5a04b306c280730f872be5cd1970a3b493bdb4855fdbc2725dd9452f1a3e9412")?,
                        y_parity: true,
                    }
                ),
                Address::from_str("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266")?,
            )},
            hash: OnceLock::new(),
        }),
        eip1559 => PooledTransaction::Eip1559(Eip1559PooledTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(1),
            max_fee_per_gas: U256::from(2),
            gas_limit: 3,
            kind: TxKind::Create,
            value: U256::from(4),
            input: Bytes::from(vec![1, 2]),
            access_list: vec![].into(),
            // SAFETY: Signature and caller address have been precomputed based on
            // `crate::edr_eth::transaction::signed::impl_test_signed_transaction_encoding_round_trip!`
            signature: unsafe { signature::Fakeable::with_address_unchecked(
                SignatureWithYParity::new(
                    SignatureWithYParityArgs {
                        r: U256::from_str("0x263b71578125bf86e9e842a920af2d941cd023893c4a452d158c87eabdf06bb9")?,
                        s: U256::from_str("0x097ff1980e38856c8e0310823e6cfc83032314f50ddd38568d3c9cf93e47d517")?,
                        y_parity: true,
                    }
                ),
                Address::from_str("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266")?,
            )},
            hash: OnceLock::new(),
        }),
        eip4844 => PooledTransaction::Eip4844(
            Eip4844::new(transaction::signed::Eip4844 {
                chain_id: 1337,
                nonce: 0,
                max_priority_fee_per_gas: U256::from(1_000_000_000),
                max_fee_per_gas: U256::from(1_000_000_000),
                max_fee_per_blob_gas: U256::from(1),
                gas_limit: 1_000_000,
                to: Address::ZERO,
                value: U256::ZERO,
                input: Bytes::from_str("0x2069b0c7")?,
                access_list: vec![].into(),
                blob_hashes: vec![B256::from_str("0x01ae39c06daecb6a178655e3fab2e56bd61e81392027947529e4def3280c546e")?],
                // SAFETY: Signature and caller address have been precomputed
                signature: unsafe { signature::Fakeable::with_address_unchecked(
                    SignatureWithYParity::new(
                        SignatureWithYParityArgs {
                            r: U256::from_str("0xaeb099417be87077fe470104f6aa73e4e473a51a6c4be62607d10e8f13f9d082")?,
                            s: U256::from_str("0x390a4c98aaecf0cfc2b27e68bdcec511dd4136356197e5937ce186af5608690b")?,
                            y_parity: true,
                        }
                    ),
                    Address::from_str("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266")?,
                )},
                hash: OnceLock::new(),
            },
            vec![fake_eip4844_blob()],
            vec![c_kzg::Bytes48::from_hex(
                "b93ab7583ad8a57b2edd262889391f37a83ab41107dc02c1a68220841379ae828343e84ac1c70fb7c2640ee3522c4c36"
            ).expect("Invalid commitment")],
            vec![c_kzg::Bytes48::from_hex(
                "86ffb073648261475af77cc902c5189bf3d33d0f63e025f23c69ac1e4cc0a7646e1a59ff8e5600f0fcc35f78fe1a4df2"
            ).expect("Invalid proof")], EnvKzgSettings::Default.get())?
        ),
        eip7702 => PooledTransaction::Eip7702(Eip7702 {
            chain_id: 31337,
            nonce: 0,
            max_priority_fee_per_gas: U256::from(1_000_000_000u64),
            max_fee_per_gas: U256::from(2_200_000_000u64),
            gas_limit: 63_000,
            to: address!("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
            value: U256::ZERO,
            input: Bytes::new(),
            access_list: Vec::new().into(),
            authorization_list: vec![
                eip7702::SignedAuthorization::new_unchecked(
                    eip7702::Authorization {
                        chain_id: U256::from(31337),
                        address: address!("0x1234567890123456789012345678901234567890"),
                        nonce: 0,
                    },
                    0,
                    U256::from_str(
                        "0xb776080626e62615e2a51a6bde9b4b4612af2627e386734f9af466ecfce19b8d",
                    )
                    .expect("R value is valid"),
                    U256::from_str(
                        "0x0d5c886f5874383826ac237ea99bfbbf601fad0fd344458296677930d51ff444",
                    )
                    .expect("S value is valid"),
                )
            ],
            // SAFETY: Signature and caller address have been precomputed from the test data in
            // `src/transaction/signed/eip7702.rs`.
            signature: unsafe { signature::Fakeable::with_address_unchecked(
                SignatureWithYParity::new(
                    SignatureWithYParityArgs {
                        r: U256::from_str("0xc6b497dd8d2b10eae25059ebc11b6228d15892c998856b04a1645cc932aad4c1")?,
                        s: U256::from_str("0x7dc710bb53d4783dbbe2ae959e4954cf8d3c9612b6ddcfd5a528a2c2250114a6")?,
                        y_parity: true,
                    }
                ),
                Address::from_str("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266")?,
            )},
            hash: OnceLock::new(),
        }),
    }
}
