use edr_eth::{
    eips,
    signature::SignatureError,
    transaction::{
        signed::{FakeSign, Sign},
        TxKind,
    },
    Address, Bytes, EvmSpecId, U256,
};
use edr_provider::{
    calculate_eip1559_fee_parameters,
    requests::validation::validate_send_transaction_request,
    spec::{FromRpcType, TransactionContext},
    time::TimeSinceEpoch,
    ProviderError, ProviderErrorForChainSpec,
};
use edr_rpc_eth::RpcTransactionRequest;
use k256::SecretKey;

use crate::{transaction::signed::L1SignedTransaction, L1ChainSpec};

/// Convenience type alias for [`edr_eth::transaction::request::Legacy`].
///
/// This allows usage like `edr_chain_l1::transaction::Legacy`.
pub type Legacy = edr_eth::transaction::request::Legacy;

/// Convenience type alias for [`edr_eth::transaction::request::Eip155`].
///
/// This allows usage like `edr_chain_l1::transaction::Eip155`.
pub type Eip155 = edr_eth::transaction::request::Eip155;

/// Convenience type alias for [`edr_eth::transaction::request::Eip2930`].
///
/// This allows usage like `edr_chain_l1::transaction::Eip2930`.
pub type Eip2930 = edr_eth::transaction::request::Eip2930;

/// Convenience type alias for [`edr_eth::transaction::request::Eip1559`].
///
/// This allows usage like `edr_chain_l1::transaction::Eip1559`.
pub type Eip1559 = edr_eth::transaction::request::Eip1559;

/// Convenience type alias for [`edr_eth::transaction::request::Eip4844`].
///
/// This allows usage like `edr_chain_l1::transaction::Eip4844`.
pub type Eip4844 = edr_eth::transaction::request::Eip4844;

/// Convenience type alias for [`edr_eth::transaction::request::Eip7702`].
///
/// This allows usage like `edr_chain_l1::transaction::Eip7702`.
pub type Eip7702 = edr_eth::transaction::request::Eip7702;

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
    pub fn authorization_list(&self) -> Option<&[eips::eip7702::SignedAuthorization]> {
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

    /// Signs the transaction request with the provided secret key.
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

impl<TimerT: Clone + TimeSinceEpoch> FromRpcType<RpcTransactionRequest, TimerT>
    for L1TransactionRequest
{
    type Context<'context> = TransactionContext<'context, L1ChainSpec, TimerT>;

    type Error = ProviderErrorForChainSpec<L1ChainSpec>;

    fn from_rpc_type(
        value: RpcTransactionRequest,
        context: Self::Context<'_>,
    ) -> Result<L1TransactionRequest, ProviderErrorForChainSpec<L1ChainSpec>> {
        let TransactionContext { data } = context;

        validate_send_transaction_request(data, &value)?;

        let RpcTransactionRequest {
            from,
            to,
            gas_price,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            gas,
            value,
            data: input,
            nonce,
            chain_id,
            access_list,
            // We ignore the transaction type
            transaction_type: _transaction_type,
            blobs: _blobs,
            blob_hashes: _blob_hashes,
            authorization_list,
        } = value;

        let chain_id = chain_id.unwrap_or_else(|| data.chain_id());
        let gas_limit = gas.unwrap_or_else(|| data.block_gas_limit());
        let input = input.map_or(Bytes::new(), Into::into);
        let nonce = nonce.map_or_else(|| data.account_next_nonce(&from), Ok)?;
        let value = value.unwrap_or(U256::ZERO);

        let current_hardfork = data.evm_spec_id();
        let request = if let Some(authorization_list) = authorization_list {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                calculate_eip1559_fee_parameters(data, max_fee_per_gas, max_priority_fee_per_gas)?;

            Self::Eip7702(Eip7702 {
                nonce,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                gas_limit,
                value,
                input,
                to: to.ok_or(ProviderError::Eip7702TransactionMissingReceiver)?,
                chain_id,
                access_list: access_list.unwrap_or_default(),
                authorization_list,
            })
        } else if current_hardfork >= EvmSpecId::LONDON
            && (gas_price.is_none()
                || max_fee_per_gas.is_some()
                || max_priority_fee_per_gas.is_some())
        {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                calculate_eip1559_fee_parameters(data, max_fee_per_gas, max_priority_fee_per_gas)?;

            Self::Eip1559(Eip1559 {
                nonce,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                gas_limit,
                value,
                input,
                kind: match to {
                    Some(to) => TxKind::Call(to),
                    None => TxKind::Create,
                },
                chain_id,
                access_list: access_list.unwrap_or_default(),
            })
        } else if let Some(access_list) = access_list {
            Self::Eip2930(Eip2930 {
                nonce,
                gas_price: gas_price.map_or_else(|| data.next_gas_price(), Ok)?,
                gas_limit,
                value,
                input,
                kind: match to {
                    Some(to) => TxKind::Call(to),
                    None => TxKind::Create,
                },
                chain_id,
                access_list,
            })
        } else {
            Self::Eip155(Eip155 {
                nonce,
                gas_price: gas_price.map_or_else(|| data.next_gas_price(), Ok)?,
                gas_limit,
                value,
                input,
                kind: match to {
                    Some(to) => TxKind::Call(to),
                    None => TxKind::Create,
                },
                chain_id,
            })
        };

        Ok(request)
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
