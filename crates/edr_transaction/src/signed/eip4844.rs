use std::sync::OnceLock;

use alloy_rlp::{Encodable as _, RlpDecodable, RlpEncodable};
use edr_chain_spec::ExecutableTransaction;
use edr_primitives::{Address, Bytes, B256, U256};
use edr_signer::{FakeableSignature, SignatureWithYParity};
use revm_primitives::{eip4844::GAS_PER_BLOB, keccak256, TxKind};

use crate::{request, utils::enveloped};

#[derive(Clone, Debug, Eq, serde::Serialize, RlpEncodable)]
pub struct Eip4844 {
    // The order of these fields determines encoding order.
    #[serde(with = "alloy_serde::quantity")]
    pub chain_id: u64,
    #[serde(with = "alloy_serde::quantity")]
    pub nonce: u64,
    #[serde(with = "alloy_serde::quantity")]
    pub max_priority_fee_per_gas: u128,
    #[serde(with = "alloy_serde::quantity")]
    pub max_fee_per_gas: u128,
    #[serde(with = "alloy_serde::quantity")]
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub access_list: edr_eip2930::AccessList,
    #[serde(with = "alloy_serde::quantity")]
    pub max_fee_per_blob_gas: u128,
    pub blob_hashes: Vec<B256>,
    #[serde(flatten)]
    pub signature: FakeableSignature<SignatureWithYParity>,
    /// Cached transaction hash
    #[rlp(default)]
    #[rlp(skip)]
    #[serde(skip)]
    pub hash: OnceLock<B256>,
    /// Cached RLP-encoding
    #[rlp(skip)]
    #[serde(skip)]
    pub rlp_encoding: OnceLock<Bytes>,
}

impl Eip4844 {
    /// The type identifier for an EIP-4844 transaction.
    pub const TYPE: u8 = request::Eip4844::TYPE;
}

impl ExecutableTransaction for Eip4844 {
    fn caller(&self) -> &Address {
        self.signature.caller()
    }

    fn gas_limit(&self) -> u64 {
        self.gas_limit
    }

    fn gas_price(&self) -> &u128 {
        &self.max_fee_per_gas
    }

    fn kind(&self) -> TxKind {
        TxKind::Call(self.to)
    }

    fn value(&self) -> &U256 {
        &self.value
    }

    fn data(&self) -> &Bytes {
        &self.input
    }

    fn nonce(&self) -> u64 {
        self.nonce
    }

    fn chain_id(&self) -> Option<u64> {
        Some(self.chain_id)
    }

    fn access_list(&self) -> Option<&[edr_eip2930::AccessListItem]> {
        Some(&self.access_list)
    }

    fn effective_gas_price(&self, block_base_fee: u128) -> Option<u128> {
        Some(
            self.max_fee_per_gas
                .min(block_base_fee + self.max_priority_fee_per_gas),
        )
    }

    fn max_fee_per_gas(&self) -> Option<&u128> {
        Some(&self.max_fee_per_gas)
    }

    fn max_priority_fee_per_gas(&self) -> Option<&u128> {
        Some(&self.max_priority_fee_per_gas)
    }

    fn blob_hashes(&self) -> &[B256] {
        &self.blob_hashes
    }

    fn max_fee_per_blob_gas(&self) -> Option<&u128> {
        Some(&self.max_fee_per_blob_gas)
    }

    fn total_blob_gas(&self) -> Option<u64> {
        Some(total_blob_gas(self))
    }

    fn authorization_list(&self) -> Option<&[edr_eip7702::SignedAuthorization]> {
        None
    }

    fn rlp_encoding(&self) -> &Bytes {
        self.rlp_encoding.get_or_init(|| {
            let mut encoded = Vec::with_capacity(1 + self.length());
            enveloped(Self::TYPE, self, &mut encoded);
            encoded.into()
        })
    }

    fn transaction_hash(&self) -> &B256 {
        self.hash.get_or_init(|| keccak256(self.rlp_encoding()))
    }
}

impl PartialEq for Eip4844 {
    fn eq(&self, other: &Self) -> bool {
        self.chain_id == other.chain_id
            && self.nonce == other.nonce
            && self.max_priority_fee_per_gas == other.max_priority_fee_per_gas
            && self.max_fee_per_gas == other.max_fee_per_gas
            && self.max_fee_per_blob_gas == other.max_fee_per_blob_gas
            && self.gas_limit == other.gas_limit
            && self.to == other.to
            && self.value == other.value
            && self.input == other.input
            && self.access_list == other.access_list
            && self.blob_hashes == other.blob_hashes
            && self.signature == other.signature
    }
}

#[derive(RlpDecodable)]
struct Decodable {
    // The order of these fields determines decoding order.
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee_per_gas: u128,
    pub max_fee_per_gas: u128,
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub access_list: edr_eip2930::AccessList,
    pub max_fee_per_blob_gas: u128,
    pub blob_hashes: Vec<B256>,
    pub signature: SignatureWithYParity,
}

impl alloy_rlp::Decodable for Eip4844 {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let transaction = Decodable::decode(buf)?;
        let request = request::Eip4844::from(&transaction);

        let signature = FakeableSignature::recover(transaction.signature, request.hash().into())
            .map_err(|_error| alloy_rlp::Error::Custom("Invalid Signature"))?;

        Ok(Self {
            chain_id: transaction.chain_id,
            nonce: transaction.nonce,
            max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
            max_fee_per_gas: transaction.max_fee_per_gas,
            gas_limit: transaction.gas_limit,
            to: transaction.to,
            value: transaction.value,
            input: transaction.input,
            access_list: transaction.access_list,
            max_fee_per_blob_gas: transaction.max_fee_per_blob_gas,
            blob_hashes: transaction.blob_hashes,
            signature,
            hash: OnceLock::new(),
            rlp_encoding: OnceLock::new(),
        })
    }
}

impl From<&Decodable> for request::Eip4844 {
    fn from(value: &Decodable) -> Self {
        Self {
            chain_id: value.chain_id,
            nonce: value.nonce,
            max_priority_fee_per_gas: value.max_priority_fee_per_gas,
            max_fee_per_gas: value.max_fee_per_gas,
            max_fee_per_blob_gas: value.max_fee_per_blob_gas,
            gas_limit: value.gas_limit,
            to: value.to,
            value: value.value,
            input: value.input.clone(),
            access_list: value.access_list.0.clone(),
            blob_hashes: value.blob_hashes.clone(),
        }
    }
}

/// Total blob gas used by the transaction.
pub fn total_blob_gas(transaction: &Eip4844) -> u64 {
    GAS_PER_BLOB * (transaction.blob_hashes.len() as u64)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use edr_signer::{SignatureWithYParity, SignatureWithYParityArgs};
    use revm_primitives::{address, b256};

    use super::*;

    // From https://github.com/ethereumjs/ethereumjs-monorepo/blob/master/packages/tx/test/eip4844.spec.ts#L68
    fn dummy_transaction() -> Eip4844 {
        let signature = SignatureWithYParity::new(SignatureWithYParityArgs {
            r: U256::from_str("0x8a83833ec07806485a4ded33f24f5cea4b8d4d24dc8f357e6d446bcdae5e58a7")
                .unwrap(),
            s: U256::from_str("0x68a2ba422a50cf84c0b5fcbda32ee142196910c97198ffd99035d920c2b557f8")
                .unwrap(),
            y_parity: false,
        });

        let request = request::Eip4844 {
            chain_id: 0x28757b3,
            nonce: 0,
            max_priority_fee_per_gas: 0x12a05f200,
            max_fee_per_gas: 0x12a05f200,
            gas_limit: 0x33450,
            to: Address::from_str("0xffb38a7a99e3e2335be83fc74b7faa19d5531243").unwrap(),
            value: U256::from(0xbc614eu64),
            input: Bytes::default(),
            access_list: Vec::new(),
            max_fee_per_blob_gas: 0xb2d05e00,
            blob_hashes: vec![B256::from_str(
                "0x01b0a4cdd5f55589f5c5b4d46c76704bb6ce95c0a8c09f77f197a57808dded28",
            )
            .unwrap()],
        };

        let signature = FakeableSignature::recover(signature, request.hash().into())
            .expect("Failed to retrieve caller");

        Eip4844 {
            chain_id: request.chain_id,
            nonce: request.nonce,
            max_priority_fee_per_gas: request.max_priority_fee_per_gas,
            max_fee_per_gas: request.max_fee_per_gas,
            gas_limit: request.gas_limit,
            to: request.to,
            value: request.value,
            input: request.input,
            access_list: request.access_list.into(),
            max_fee_per_blob_gas: request.max_fee_per_blob_gas,
            blob_hashes: request.blob_hashes,
            signature,
            hash: OnceLock::new(),
            rlp_encoding: OnceLock::new(),
        }
    }

    #[test]
    fn eip4844_signed_transaction_encoding() {
        // From https://github.com/ethereumjs/ethereumjs-monorepo/blob/master/packages/tx/test/eip4844.spec.ts#L86
        let expected =
            hex::decode("f89b84028757b38085012a05f20085012a05f2008303345094ffb38a7a99e3e2335be83fc74b7faa19d553124383bc614e80c084b2d05e00e1a001b0a4cdd5f55589f5c5b4d46c76704bb6ce95c0a8c09f77f197a57808dded2880a08a83833ec07806485a4ded33f24f5cea4b8d4d24dc8f357e6d446bcdae5e58a7a068a2ba422a50cf84c0b5fcbda32ee142196910c97198ffd99035d920c2b557f8")
                .unwrap();

        let signed = dummy_transaction();
        let encoded = alloy_rlp::encode(&signed);
        assert_eq!(expected, encoded);
    }

    #[test]
    fn eip4844_signed_transaction_hash() {
        // From https://github.com/ethereumjs/ethereumjs-monorepo/blob/master/packages/tx/test/eip4844.spec.ts#L86
        let expected =
            B256::from_str("0xe5e02be0667b6d31895d1b5a8b916a6761cbc9865225c6144a3e2c50936d173e")
                .unwrap();

        let signed = dummy_transaction();
        assert_eq!(expected, *signed.transaction_hash());
    }

    #[test]
    fn recover() -> anyhow::Result<()> {
        // From https://github.com/NomicFoundation/edr/issues/341#issuecomment-2039360056
        const CALLER: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

        let request = request::Eip4844 {
            chain_id: 1337,
            nonce: 0,
            max_priority_fee_per_gas: 0x3b9aca00,
            max_fee_per_gas: 0x3b9aca00,
            gas_limit: 1000000,
            to: Address::ZERO,
            value: U256::ZERO,
            input: Bytes::from_str("0x2069b0c7")?,
            access_list: Vec::new(),
            max_fee_per_blob_gas: 1,
            blob_hashes: vec![b256!(
                "01ae39c06daecb6a178655e3fab2e56bd61e81392027947529e4def3280c546e"
            )],
        };

        let signature = FakeableSignature::recover(
            SignatureWithYParity::new(SignatureWithYParityArgs {
                r: U256::from_str(
                    "0xaeb099417be87077fe470104f6aa73e4e473a51a6c4be62607d10e8f13f9d082",
                )?,
                s: U256::from_str(
                    "0x390a4c98aaecf0cfc2b27e68bdcec511dd4136356197e5937ce186af5608690b",
                )?,
                y_parity: true,
            }),
            request.hash().into(),
        )
        .expect("Failed to recover caller");

        let transaction = Eip4844 {
            chain_id: request.chain_id,
            nonce: request.nonce,
            max_priority_fee_per_gas: request.max_priority_fee_per_gas,
            max_fee_per_gas: request.max_fee_per_gas,
            gas_limit: request.gas_limit,
            to: request.to,
            value: request.value,
            input: request.input,
            access_list: request.access_list.into(),
            max_fee_per_blob_gas: request.max_fee_per_blob_gas,
            blob_hashes: request.blob_hashes,
            signature,
            hash: OnceLock::new(),
            rlp_encoding: OnceLock::new(),
        };

        assert_eq!(*transaction.caller(), CALLER);

        Ok(())
    }
}
