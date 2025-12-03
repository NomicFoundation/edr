use edr_chain_l1::{
    rpc::{call::L1CallRequest, transaction::L1RpcTransactionRequest},
    L1TransactionRequest,
};
use edr_chain_spec::EvmSpecId;
use edr_primitives::{Address, Bytes, U256};
use edr_provider::{
    calculate_eip1559_fee_parameters,
    requests::validation::{validate_call_request, validate_send_transaction_request},
    spec::{CallContext, FromRpcType, TransactionContext},
    time::TimeSinceEpoch,
    ProviderError, ProviderErrorForChainSpec,
};
use edr_signer::{FakeSign, SecretKey, Sign, SignatureError};
use edr_transaction::TxKind;

use crate::{transaction::SignedTransactionWithFallbackToPostEip155, GenericChainSpec};

/// Container type for various Ethereum transaction requests.
// NOTE: This is a newtype only because the default FromRpcType implementation
// provides an error of ProviderError<L1ChainSpec> specifically. Despite us
// wanting the same logic, we need to use our own type and copy the
// implementation.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GenericTransactionRequest(L1TransactionRequest);

impl From<L1TransactionRequest> for GenericTransactionRequest {
    fn from(value: L1TransactionRequest) -> Self {
        Self(value)
    }
}

impl FakeSign for GenericTransactionRequest {
    type Signed = SignedTransactionWithFallbackToPostEip155;

    fn fake_sign(self, sender: Address) -> SignedTransactionWithFallbackToPostEip155 {
        <L1TransactionRequest as FakeSign>::fake_sign(self.0, sender).into()
    }
}

impl Sign for GenericTransactionRequest {
    type Signed = SignedTransactionWithFallbackToPostEip155;

    unsafe fn sign_for_sender_unchecked(
        self,
        secret_key: &SecretKey,
        caller: Address,
    ) -> Result<SignedTransactionWithFallbackToPostEip155, SignatureError> {
        // SAFETY: The safety concern is propagated in the function signature.
        unsafe {
            <L1TransactionRequest as Sign>::sign_for_sender_unchecked(self.0, secret_key, caller)
        }
        .map(Into::into)
    }
}

impl<TimerT: Clone + TimeSinceEpoch> FromRpcType<L1CallRequest, TimerT>
    for GenericTransactionRequest
{
    type Context<'context> = CallContext<'context, GenericChainSpec, TimerT>;

    type Error = ProviderErrorForChainSpec<GenericChainSpec>;

    fn from_rpc_type(
        value: L1CallRequest,
        context: Self::Context<'_>,
    ) -> Result<
        crate::transaction::GenericTransactionRequest,
        ProviderErrorForChainSpec<GenericChainSpec>,
    > {
        let CallContext {
            data,
            block_spec,
            state_overrides,
            default_gas_price_fn,
            max_fees_fn,
        } = context;

        validate_call_request::<GenericChainSpec, TimerT>(data.hardfork(), &value, block_spec)?;

        let L1CallRequest {
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
        let gas_limit = gas.unwrap_or_else(|| {
            data.transaction_gas_cap()
                .unwrap_or_else(|| data.block_gas_limit())
        });
        let input = input.map_or(Bytes::new(), Bytes::from);
        let nonce = data.nonce(&sender, Some(block_spec), state_overrides)?;
        let value = value.unwrap_or(U256::ZERO);

        let evm_spec_id = data.evm_spec_id();
        let request = if evm_spec_id < EvmSpecId::LONDON || gas_price.is_some() {
            let gas_price = gas_price.map_or_else(|| default_gas_price_fn(data), Ok)?;
            match access_list {
                Some(access_list) if evm_spec_id >= EvmSpecId::BERLIN => {
                    L1TransactionRequest::Eip2930(edr_chain_l1::request::Eip2930 {
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
                _ => L1TransactionRequest::Eip155(edr_chain_l1::request::Eip155 {
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
                L1TransactionRequest::Eip7702(edr_chain_l1::request::Eip7702 {
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
                L1TransactionRequest::Eip1559(edr_chain_l1::request::Eip1559 {
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

impl<TimerT: Clone + TimeSinceEpoch> FromRpcType<L1RpcTransactionRequest, TimerT>
    for GenericTransactionRequest
{
    type Context<'context> = TransactionContext<'context, GenericChainSpec, TimerT>;

    type Error = ProviderErrorForChainSpec<GenericChainSpec>;

    fn from_rpc_type(
        value: L1RpcTransactionRequest,
        context: Self::Context<'_>,
    ) -> Result<
        crate::transaction::GenericTransactionRequest,
        ProviderErrorForChainSpec<GenericChainSpec>,
    > {
        let TransactionContext { data } = context;

        validate_send_transaction_request(data, &value)?;

        let L1RpcTransactionRequest {
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
        let gas_limit = gas.unwrap_or_else(|| {
            data.transaction_gas_cap()
                .unwrap_or_else(|| data.block_gas_limit())
        });
        let input = input.map_or(Bytes::new(), Into::into);
        let nonce = nonce.map_or_else(|| data.account_next_nonce(&from), Ok)?;
        let value = value.unwrap_or(U256::ZERO);

        let current_hardfork = data.evm_spec_id();
        let request = if let Some(authorization_list) = authorization_list {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                calculate_eip1559_fee_parameters(data, max_fee_per_gas, max_priority_fee_per_gas)?;

            L1TransactionRequest::Eip7702(edr_chain_l1::request::Eip7702 {
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

            L1TransactionRequest::Eip1559(edr_chain_l1::request::Eip1559 {
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
            L1TransactionRequest::Eip2930(edr_chain_l1::request::Eip2930 {
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
            L1TransactionRequest::Eip155(edr_chain_l1::request::Eip155 {
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
