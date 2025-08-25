use std::sync::OnceLock;

use alloy_rlp::RlpEncodable;
use edr_signer::{
    public_key_to_address, FakeableSignature, SecretKey, SignatureError, SignatureWithYParity,
};
use revm_primitives::keccak256;

use crate::{
    transaction::{self, ComputeTransactionHash},
    utils::envelop_bytes,
    Address, Bytes, B256, U256,
};

/// An [EIP-7702](https://eips.ethereum.org/EIPS/eip-7702) transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct Eip7702 {
    // The order of these fields determines encoding order.
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee_per_gas: u128,
    pub max_fee_per_gas: u128,
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub access_list: Vec<edr_eip2930::AccessListItem>,
    pub authorization_list: Vec<edr_eip7702::SignedAuthorization>,
}

impl Eip7702 {
    pub const TYPE: u8 = 4;

    /// Signs the transaction with the provided secret key.
    pub fn sign(
        self,
        secret_key: &SecretKey,
    ) -> Result<transaction::signed::Eip7702, SignatureError> {
        let caller = public_key_to_address(secret_key.public_key());

        // SAFETY: The caller is derived from the secret key.
        unsafe { self.sign_for_sender_unchecked(secret_key, caller) }
    }

    /// Signs the transaction with the provided secret key, belonging to the
    /// provided caller's address.
    ///
    /// # Safety
    ///
    /// The `caller` and `secret_key` must correspond to the same account.
    pub unsafe fn sign_for_sender_unchecked(
        self,
        secret_key: &SecretKey,
        caller: Address,
    ) -> Result<transaction::signed::Eip7702, SignatureError> {
        let hash = self.compute_transaction_hash();
        let signature = SignatureWithYParity::with_message(hash, secret_key)?;

        Ok(transaction::signed::Eip7702 {
            chain_id: self.chain_id,
            nonce: self.nonce,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas,
            max_fee_per_gas: self.max_fee_per_gas,
            gas_limit: self.gas_limit,
            to: self.to,
            value: self.value,
            input: self.input,
            access_list: self.access_list.into(),
            authorization_list: self.authorization_list,
            // SAFETY: The safety concern is propagated in the function signature.
            signature: unsafe { FakeableSignature::with_address_unchecked(signature, caller) },
            hash: OnceLock::new(),
            rlp_encoding: OnceLock::new(),
        })
    }

    pub fn fake_sign(self, address: Address) -> transaction::signed::Eip7702 {
        transaction::signed::Eip7702 {
            chain_id: self.chain_id,
            nonce: self.nonce,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas,
            max_fee_per_gas: self.max_fee_per_gas,
            gas_limit: self.gas_limit,
            to: self.to,
            value: self.value,
            input: self.input,
            access_list: self.access_list.into(),
            authorization_list: self.authorization_list,
            signature: FakeableSignature::fake(address, None),
            hash: OnceLock::new(),
            rlp_encoding: OnceLock::new(),
        }
    }
}

impl ComputeTransactionHash for Eip7702 {
    fn compute_transaction_hash(&self) -> B256 {
        let encoded = alloy_rlp::encode(self);
        let enveloped = envelop_bytes(Eip7702::TYPE, &encoded);

        keccak256(enveloped)
    }
}

#[cfg(test)]
mod tests {
    mod expectation {
        use core::str::FromStr as _;

        use hex::FromHexError;

        use super::*;

        pub const REQUEST_HASH: B256 =
            b256!("056880940567cb424c9959fc670bca016107f9b305158837ef1b0c721e1cbb65");

        pub fn raw() -> Result<Vec<u8>, FromHexError> {
            hex::decode(
                "f889827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85ef85c827a699412345678901234567890123456789012345678900101a0eb775e0a2b7a15ea4938921e1ab255c84270e25c2c384b2adc32c73cd70273d6a046b9bec1961318a644db6cd9c7fc4e8d7c6f40d9165fc8958f3aff2216ed6f7c",
            )
        }

        // Test vector generated using secret key in `dummy_secret_key`.
        pub fn request() -> anyhow::Result<transaction::request::Eip7702> {
            const CHAIN_ID: u64 = 0x7a69;

            let request = transaction::request::Eip7702 {
                chain_id: CHAIN_ID,
                nonce: 0,
                max_priority_fee_per_gas: 1_000_000_000,
                max_fee_per_gas: 2_200_000_000,
                gas_limit: 63_000,
                to: address!("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
                value: U256::ZERO,
                input: Bytes::new(),
                access_list: Vec::new(),
                authorization_list: vec![edr_eip7702::SignedAuthorization::new_unchecked(
                    edr_eip7702::Authorization {
                        chain_id: U256::from(CHAIN_ID),
                        address: address!("0x1234567890123456789012345678901234567890"),
                        nonce: 1,
                    },
                    1,
                    U256::from_str(
                        "0xeb775e0a2b7a15ea4938921e1ab255c84270e25c2c384b2adc32c73cd70273d6",
                    )
                    .expect("R value is valid"),
                    U256::from_str(
                        "0x46b9bec1961318a644db6cd9c7fc4e8d7c6f40d9165fc8958f3aff2216ed6f7c",
                    )
                    .expect("S value is valid"),
                )],
            };
            Ok(request)
        }
    }

    use revm_primitives::{address, b256};

    use super::*;

    #[test]
    fn encoding() -> anyhow::Result<()> {
        let request = expectation::request()?;

        let encoded = alloy_rlp::encode(&request);
        let expected = expectation::raw()?;
        assert_eq!(encoded, expected);

        Ok(())
    }

    #[test]
    fn transaction_hash() -> anyhow::Result<()> {
        let request = expectation::request()?;

        let request_hash = request.compute_transaction_hash();
        assert_eq!(request_hash, expectation::REQUEST_HASH);

        Ok(())
    }
}
