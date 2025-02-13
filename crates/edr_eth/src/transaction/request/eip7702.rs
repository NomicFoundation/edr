use std::sync::OnceLock;

use alloy_rlp::RlpEncodable;
use revm_primitives::keccak256;

use crate::{
    eips::eip7702,
    signature::{self, public_key_to_address, SecretKey, SignatureError, SignatureWithYParity},
    transaction::{self, ComputeTransactionHash},
    utils::envelop_bytes,
    AccessListItem, Address, Bytes, B256, U256,
};

/// An [EIP-7702](https://eips.ethereum.org/EIPS/eip-7702) transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct Eip7702 {
    // The order of these fields determines encoding order.
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub access_list: Vec<AccessListItem>,
    pub authorization_list: Vec<eip7702::SignedAuthorization>,
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
            signature: signature::Fakeable::with_address_unchecked(signature, caller),
            hash: OnceLock::new(),
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
            signature: signature::Fakeable::fake(address, None),
            hash: OnceLock::new(),
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
            b256!("b484d448147b9a6cafc732e01b89ee4e7d8bb783a03f5cbdd967d7bdaa945a99");

        pub fn raw() -> Result<Vec<u8>, FromHexError> {
            hex::decode("04f8cc827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85ef85c827a699412345678901234567890123456789012345678908080a0b776080626e62615e2a51a6bde9b4b4612af2627e386734f9af466ecfce19b8da00d5c886f5874383826ac237ea99bfbbf601fad0fd344458296677930d51ff44480a0a5f83207382081e8de07113af9ba61e4b41c9ae306edc55a2787996611d1ade9a0082f979b985ea64b4755344b57bcd66ade2b840e8be2036101d9cf23a8548412")
        }

        // Test vector generated using secret key in `dummy_secret_key`.
        pub fn request() -> anyhow::Result<transaction::request::Eip7702> {
            const CHAIN_ID: u64 = 0x7a69;

            let request = transaction::request::Eip7702 {
                chain_id: CHAIN_ID,
                nonce: 0,
                max_priority_fee_per_gas: U256::from(1_000_000_000u64),
                max_fee_per_gas: U256::from(2_200_000_000u64),
                gas_limit: 63_000,
                to: address!("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
                value: U256::ZERO,
                input: Bytes::new(),
                access_list: Vec::new(),
                authorization_list: vec![eip7702::SignedAuthorization::new_unchecked(
                    eip7702::Authorization {
                        chain_id: U256::from(CHAIN_ID),
                        address: address!("0x1234567890123456789012345678901234567890"),
                        nonce: 0,
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
        assert_eq!(*request_hash, expectation::REQUEST_HASH);

        Ok(())
    }
}
