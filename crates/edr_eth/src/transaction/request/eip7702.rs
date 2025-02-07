use std::sync::OnceLock;

use alloy_rlp::RlpEncodable;
use k256::SecretKey;
use revm_primitives::keccak256;

use crate::{
    signature::{self, public_key_to_address, SignatureError, SignatureWithYParity},
    transaction::{self, ComputeTransactionHash},
    utils::envelop_bytes,
    AccessListItem, Address, Bytes, SignedAuthorization, B256, U256,
};

/// An [EIP-7702](https://eips.ethereum.org/EIPS/eip-7702) transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct Eip7702 {
    // The order of these fields determines encoding order.
    pub chain_id: u64,
    pub nonce: u64,
    pub gas_limit: u64,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub to: Address,
    pub value: U256,
    pub access_list: Vec<AccessListItem>,
    pub authorization_list: Vec<SignedAuthorization>,
    pub input: Bytes,
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
        let signature = SignatureWithYParity::new(hash, secret_key)?;

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
