use edr_evm_spec::EvmSpecId;
use edr_provider::{
    calculate_eip1559_fee_parameters,
    requests::validation::{validate_call_request, validate_send_transaction_request},
    spec::{CallContext, FromRpcType, TransactionContext},
    time::TimeSinceEpoch,
    ProviderError, ProviderErrorForChainSpec,
};
use edr_rpc_eth::{CallRequest, TransactionRequest};
use edr_signer::{FakeSign, SecretKey, Sign, SignatureError};
use edr_transaction::{Address, Bytes, TxKind, U256};

use crate::{transaction::SignedWithFallbackToPostEip155, GenericChainSpec};

/// Container type for various Ethereum transaction requests.
// NOTE: This is a newtype only because the default FromRpcType implementation
// provides an error of ProviderError<L1ChainSpec> specifically. Despite us
// wanting the same logic, we need to use our own type and copy the
// implementation.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Request(edr_chain_l1::Request);

impl From<edr_chain_l1::Request> for Request {
    fn from(value: edr_chain_l1::Request) -> Self {
        Self(value)
    }
}

impl FakeSign for Request {
    type Signed = SignedWithFallbackToPostEip155;

    fn fake_sign(self, sender: Address) -> SignedWithFallbackToPostEip155 {
        <edr_chain_l1::Request as FakeSign>::fake_sign(self.0, sender).into()
    }
}

impl Sign for Request {
    type Signed = SignedWithFallbackToPostEip155;

    unsafe fn sign_for_sender_unchecked(
        self,
        secret_key: &SecretKey,
        caller: Address,
    ) -> Result<SignedWithFallbackToPostEip155, SignatureError> {
        // SAFETY: The safety concern is propagated in the function signature.
        unsafe {
            <edr_chain_l1::Request as Sign>::sign_for_sender_unchecked(self.0, secret_key, caller)
        }
        .map(Into::into)
    }
}

impl<TimerT: Clone + TimeSinceEpoch> FromRpcType<CallRequest, TimerT> for Request {
    type Context<'context> = CallContext<'context, GenericChainSpec, TimerT>;

    type Error = ProviderErrorForChainSpec<GenericChainSpec>;

    fn from_rpc_type(
        value: CallRequest,
        context: Self::Context<'_>,
    ) -> Result<crate::transaction::Request, ProviderErrorForChainSpec<GenericChainSpec>> {
        let CallContext {
            data,
            block_spec,
            state_overrides,
            default_gas_price_fn,
            max_fees_fn,
        } = context;

        validate_call_request::<GenericChainSpec, TimerT>(data.hardfork(), &value, block_spec)?;

        let CallRequest {
            from,
            to,
            gas,
            gas_price,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            value,
            data: input,
            access_list,
            // We ignore the transaction type
            transaction_type: _transaction_type,
            blobs: _blobs,
            blob_hashes: _blob_hashes,
            authorization_list,
        } = value;

        let chain_id = data.chain_id_at_block_spec(block_spec)?;
        let sender = from.unwrap_or_else(|| data.default_caller());
        let gas_limit = gas.unwrap_or_else(|| data.block_gas_limit());
        let input = input.map_or(Bytes::new(), Bytes::from);
        let nonce = data.nonce(&sender, Some(block_spec), state_overrides)?;
        let value = value.unwrap_or(U256::ZERO);

        let evm_spec_id = data.evm_spec_id();
        let request = if evm_spec_id < EvmSpecId::LONDON || gas_price.is_some() {
            let gas_price = gas_price.map_or_else(|| default_gas_price_fn(data), Ok)?;
            match access_list {
                Some(access_list) if evm_spec_id >= EvmSpecId::BERLIN => {
                    edr_chain_l1::Request::Eip2930(edr_chain_l1::request::Eip2930 {
                        nonce,
                        gas_price,
                        gas_limit,
                        value,
                        input,
                        kind: to.into(),
                        chain_id,
                        access_list,
                    })
                }
                _ => edr_chain_l1::Request::Eip155(edr_chain_l1::request::Eip155 {
                    nonce,
                    gas_price,
                    gas_limit,
                    kind: to.into(),
                    value,
                    input,
                    chain_id,
                }),
            }
        } else {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                max_fees_fn(data, block_spec, max_fee_per_gas, max_priority_fee_per_gas)?;

            if let Some(authorization_list) = authorization_list {
                edr_chain_l1::Request::Eip7702(edr_chain_l1::request::Eip7702 {
                    chain_id,
                    nonce,
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    gas_limit,
                    to: to.ok_or(ProviderError::Eip7702TransactionMissingReceiver)?,
                    value,
                    input,
                    access_list: access_list.unwrap_or_default(),
                    authorization_list,
                })
            } else {
                edr_chain_l1::Request::Eip1559(edr_chain_l1::request::Eip1559 {
                    chain_id,
                    nonce,
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    gas_limit,
                    kind: to.into(),
                    value,
                    input,
                    access_list: access_list.unwrap_or_default(),
                })
            }
        };

        Ok(request.into())
    }
}

impl<TimerT: Clone + TimeSinceEpoch> FromRpcType<TransactionRequest, TimerT> for Request {
    type Context<'context> = TransactionContext<'context, GenericChainSpec, TimerT>;

    type Error = ProviderErrorForChainSpec<GenericChainSpec>;

    fn from_rpc_type(
        value: TransactionRequest,
        context: Self::Context<'_>,
    ) -> Result<crate::transaction::Request, ProviderErrorForChainSpec<GenericChainSpec>> {
        let TransactionContext { data } = context;

        validate_send_transaction_request(data, &value)?;

        let TransactionRequest {
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

            edr_chain_l1::Request::Eip7702(edr_chain_l1::request::Eip7702 {
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

            edr_chain_l1::Request::Eip1559(edr_chain_l1::request::Eip1559 {
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
            edr_chain_l1::Request::Eip2930(edr_chain_l1::request::Eip2930 {
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
            edr_chain_l1::Request::Eip155(edr_chain_l1::request::Eip155 {
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

        Ok(request.into())
    }
}
