//! Types for Ethereum L1 transaction requests.

use edr_signer::{Address, FakeSign, SecretKey, Sign, SignatureError};
pub use edr_transaction::request::{Eip155, Eip1559, Eip2930, Eip4844, Eip7702, Legacy};

use crate::L1SignedTransaction;

/// Container type for various Ethereum transaction requests
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum L1TransactionRequest {
    /// A legacy transaction request
    Legacy(Legacy),
    /// An EIP-155 transaction request
    Eip155(Eip155),
    /// An EIP-2930 transaction request
    Eip2930(Eip2930),
    /// An EIP-1559 transaction request
    Eip1559(Eip1559),
    /// An EIP-4844 transaction request
    Eip4844(Eip4844),
    /// An EIP-7702 transaction request
    Eip7702(Eip7702),
}

impl L1TransactionRequest {
    /// Retrieves the instance's authorization list (EIP-7702).
    pub fn authorization_list(&self) -> Option<&[edr_eip7702::SignedAuthorization]> {
        match self {
            L1TransactionRequest::Eip7702(transaction) => Some(&transaction.authorization_list),
            L1TransactionRequest::Legacy(_)
            | L1TransactionRequest::Eip155(_)
            | L1TransactionRequest::Eip2930(_)
            | L1TransactionRequest::Eip1559(_)
            | L1TransactionRequest::Eip4844(_) => None,
        }
    }

    /// Retrieves the instance's chain ID.
    pub fn chain_id(&self) -> Option<u64> {
        match self {
            L1TransactionRequest::Legacy(_) => None,
            L1TransactionRequest::Eip155(transaction) => Some(transaction.chain_id),
            L1TransactionRequest::Eip2930(transaction) => Some(transaction.chain_id),
            L1TransactionRequest::Eip1559(transaction) => Some(transaction.chain_id),
            L1TransactionRequest::Eip4844(transaction) => Some(transaction.chain_id),
            L1TransactionRequest::Eip7702(transaction) => Some(transaction.chain_id),
        }
    }

    /// Retrieves the instance's gas price.
    pub fn gas_price(&self) -> &u128 {
        match self {
            L1TransactionRequest::Legacy(transaction) => &transaction.gas_price,
            L1TransactionRequest::Eip155(transaction) => &transaction.gas_price,
            L1TransactionRequest::Eip2930(transaction) => &transaction.gas_price,
            L1TransactionRequest::Eip1559(transaction) => &transaction.max_fee_per_gas,
            L1TransactionRequest::Eip4844(transaction) => &transaction.max_fee_per_gas,
            L1TransactionRequest::Eip7702(transaction) => &transaction.max_fee_per_gas,
        }
    }

    /// Retrieves the instance's max fee per gas, if it exists.
    pub fn max_fee_per_gas(&self) -> Option<&u128> {
        match self {
            L1TransactionRequest::Legacy(_)
            | L1TransactionRequest::Eip155(_)
            | L1TransactionRequest::Eip2930(_) => None,
            L1TransactionRequest::Eip1559(transaction) => Some(&transaction.max_fee_per_gas),
            L1TransactionRequest::Eip4844(transaction) => Some(&transaction.max_fee_per_gas),
            L1TransactionRequest::Eip7702(transaction) => Some(&transaction.max_fee_per_gas),
        }
    }

    /// Retrieves the instance's max priority fee per gas, if it exists.
    pub fn max_priority_fee_per_gas(&self) -> Option<&u128> {
        match self {
            L1TransactionRequest::Legacy(_)
            | L1TransactionRequest::Eip155(_)
            | L1TransactionRequest::Eip2930(_) => None,
            L1TransactionRequest::Eip1559(transaction) => {
                Some(&transaction.max_priority_fee_per_gas)
            }
            L1TransactionRequest::Eip4844(transaction) => {
                Some(&transaction.max_priority_fee_per_gas)
            }
            L1TransactionRequest::Eip7702(transaction) => {
                Some(&transaction.max_priority_fee_per_gas)
            }
        }
    }

    /// Retrieves the instance's nonce.
    pub fn nonce(&self) -> u64 {
        match self {
            L1TransactionRequest::Legacy(transaction) => transaction.nonce,
            L1TransactionRequest::Eip155(transaction) => transaction.nonce,
            L1TransactionRequest::Eip2930(transaction) => transaction.nonce,
            L1TransactionRequest::Eip1559(transaction) => transaction.nonce,
            L1TransactionRequest::Eip4844(transaction) => transaction.nonce,
            L1TransactionRequest::Eip7702(transaction) => transaction.nonce,
        }
    }

    pub fn sign(self, secret_key: &SecretKey) -> Result<L1SignedTransaction, SignatureError> {
        Ok(match self {
            L1TransactionRequest::Legacy(transaction) => transaction.sign(secret_key)?.into(),
            L1TransactionRequest::Eip155(transaction) => transaction.sign(secret_key)?.into(),
            L1TransactionRequest::Eip2930(transaction) => transaction.sign(secret_key)?.into(),
            L1TransactionRequest::Eip1559(transaction) => transaction.sign(secret_key)?.into(),
            L1TransactionRequest::Eip4844(transaction) => transaction.sign(secret_key)?.into(),
            L1TransactionRequest::Eip7702(transaction) => transaction.sign(secret_key)?.into(),
        })
    }
}

impl FakeSign for L1TransactionRequest {
    type Signed = L1SignedTransaction;

    fn fake_sign(self, sender: Address) -> L1SignedTransaction {
        match self {
            L1TransactionRequest::Legacy(transaction) => transaction.fake_sign(sender).into(),
            L1TransactionRequest::Eip155(transaction) => transaction.fake_sign(sender).into(),
            L1TransactionRequest::Eip2930(transaction) => transaction.fake_sign(sender).into(),
            L1TransactionRequest::Eip1559(transaction) => transaction.fake_sign(sender).into(),
            L1TransactionRequest::Eip4844(transaction) => transaction.fake_sign(sender).into(),
            L1TransactionRequest::Eip7702(transaction) => transaction.fake_sign(sender).into(),
        }
    }
}

impl Sign for L1TransactionRequest {
    type Signed = L1SignedTransaction;

    unsafe fn sign_for_sender_unchecked(
        self,
        secret_key: &SecretKey,
        caller: Address,
    ) -> Result<L1SignedTransaction, SignatureError> {
        Ok(match self {
            L1TransactionRequest::Legacy(transaction) => {
                // SAFETY: The safety concern is propagated in the function signature.
                unsafe { transaction.sign_for_sender_unchecked(secret_key, caller) }?.into()
            }
            L1TransactionRequest::Eip155(transaction) => {
                // SAFETY: The safety concern is propagated in the function signature.
                unsafe { transaction.sign_for_sender_unchecked(secret_key, caller) }?.into()
            }
            L1TransactionRequest::Eip2930(transaction) => {
                // SAFETY: The safety concern is propagated in the function signature.
                unsafe { transaction.sign_for_sender_unchecked(secret_key, caller) }?.into()
            }
            L1TransactionRequest::Eip1559(transaction) => {
                // SAFETY: The safety concern is propagated in the function signature.
                unsafe { transaction.sign_for_sender_unchecked(secret_key, caller) }?.into()
            }
            L1TransactionRequest::Eip4844(transaction) => {
                // SAFETY: The safety concern is propagated in the function signature.
                unsafe { transaction.sign_for_sender_unchecked(secret_key, caller) }?.into()
            }
            L1TransactionRequest::Eip7702(transaction) => {
                // SAFETY: The safety concern is propagated in the function signature.
                unsafe { transaction.sign_for_sender_unchecked(secret_key, caller) }?.into()
            }
        })
    }
}
