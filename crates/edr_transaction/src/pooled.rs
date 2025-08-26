/// EIP-4844 pooled transaction types
pub mod eip4844;

pub use self::eip4844::Eip4844;

pub type Legacy = super::signed::Legacy;
pub type Eip155 = super::signed::Eip155;
pub type Eip2930 = super::signed::Eip2930;
pub type Eip1559 = super::signed::Eip1559;
pub type Eip7702 = super::signed::Eip7702;

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::OnceLock};

    use alloy_rlp::Decodable;
    use c_kzg::BYTES_PER_BLOB;

    use super::*;
    use crate::{Address, Bytes, B256, U256};

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
        pre_eip155 => PooledTransaction::PreEip155Legacy(Legacy {
            nonce: 0,
            gas_price: 1,
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
            rlp_encoding: OnceLock::new(),
        }),
        post_eip155 => PooledTransaction::PostEip155Legacy(Eip155 {
            nonce: 0,
            gas_price: 1,
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
            rlp_encoding: OnceLock::new(),
        }),
        eip2930 => PooledTransaction::Eip2930(Eip2930 {
            chain_id: 1,
            nonce: 0,
            gas_price: 1,
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
            rlp_encoding: OnceLock::new(),
        }),
        eip1559 => PooledTransaction::Eip1559(Eip1559 {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: 1,
            max_fee_per_gas: 2,
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
            rlp_encoding: OnceLock::new(),
        }),
        eip4844 => PooledTransaction::Eip4844(
            Eip4844::new(transaction::signed::Eip4844 {
                chain_id: 1337,
                nonce: 0,
                max_priority_fee_per_gas: 1_000_000_000,
                max_fee_per_gas: 1_000_000_000,
                max_fee_per_blob_gas: 1,
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
                rlp_encoding: OnceLock::new(),
            },
            vec![fake_eip4844_blob()],
            vec![c_kzg::Bytes48::from_hex(
                "b93ab7583ad8a57b2edd262889391f37a83ab41107dc02c1a68220841379ae828343e84ac1c70fb7c2640ee3522c4c36"
            ).expect("Invalid commitment")],
            vec![c_kzg::Bytes48::from_hex(
                "86ffb073648261475af77cc902c5189bf3d33d0f63e025f23c69ac1e4cc0a7646e1a59ff8e5600f0fcc35f78fe1a4df2"
            ).expect("Invalid proof")], c_kzg::ethereum_kzg_settings(0))?
        ),
        eip7702 => PooledTransaction::Eip7702(Eip7702 {
            chain_id: 31337,
            nonce: 0,
            max_priority_fee_per_gas: 1_000_000_000,
            max_fee_per_gas: 2_200_000_000,
            gas_limit: 63_000,
            to: address!("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
            value: U256::ZERO,
            input: Bytes::new(),
            access_list: Vec::new().into(),
            authorization_list: vec![
                edr_eip7702::SignedAuthorization::new_unchecked(
                    edr_eip7702::Authorization {
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
            rlp_encoding: OnceLock::new(),
        }),
    }
}
