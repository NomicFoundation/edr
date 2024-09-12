use edr_eth::{
    signature::{SecretKey, SignatureError},
    transaction::{
        request::{Eip155, Eip1559, Eip2930},
        signed::{FakeSign, Sign},
        ExecutableTransaction as _, Request, Signed,
    },
    Address, Bytes, SpecId, B256, U256,
};
use edr_evm::{blockchain::BlockchainError, BlockAndTotalDifficulty};
use edr_provider::{
    requests::validation::{validate_call_request, validate_send_transaction_request},
    spec::{CallContext, ChainSpec, FromRpcType, TransactionContext},
    time::TimeSinceEpoch,
    ProviderData, ProviderError, ProviderSpec,
};
use edr_rpc_eth::{CallRequest, TransactionRequest};

use crate::{transaction::SignedWithFallbackToPostEip155, GenericChainSpec};

impl From<edr_eth::transaction::pooled::PooledTransaction> for SignedWithFallbackToPostEip155 {
    fn from(value: edr_eth::transaction::pooled::PooledTransaction) -> Self {
        edr_eth::transaction::Signed::from(value).into()
    }
}

impl FakeSign<SignedWithFallbackToPostEip155> for Request {
    fn fake_sign(self, sender: edr_eth::Address) -> SignedWithFallbackToPostEip155 {
        <Self as FakeSign<Signed>>::fake_sign(self, sender).into()
    }
}

impl Sign<SignedWithFallbackToPostEip155> for Request {
    unsafe fn sign_for_sender_unchecked(
        self,
        secret_key: &SecretKey,
        caller: Address,
    ) -> Result<SignedWithFallbackToPostEip155, SignatureError> {
        <Self as Sign<Signed>>::sign_for_sender_unchecked(self, secret_key, caller).map(Into::into)
    }
}

impl<BlockchainErrorT, ChainSpecT: ChainSpec>
    From<BlockAndTotalDifficulty<ChainSpecT, BlockchainErrorT>> for crate::rpc::block::Block<B256>
{
    fn from(value: BlockAndTotalDifficulty<ChainSpecT, BlockchainErrorT>) -> Self {
        let transactions = value
            .block
            .transactions()
            .iter()
            .map(|tx| *tx.transaction_hash())
            .collect();

        let header = value.block.header();
        crate::rpc::block::Block {
            hash: Some(*value.block.hash()),
            parent_hash: header.parent_hash,
            sha3_uncles: header.ommers_hash,
            state_root: header.state_root,
            transactions_root: header.transactions_root,
            receipts_root: header.receipts_root,
            number: Some(header.number),
            gas_used: header.gas_used,
            gas_limit: header.gas_limit,
            extra_data: header.extra_data.clone(),
            logs_bloom: header.logs_bloom,
            timestamp: header.timestamp,
            difficulty: header.difficulty,
            total_difficulty: value.total_difficulty,
            uncles: value.block.ommer_hashes().to_vec(),
            transactions,
            size: value.block.rlp_size(),
            mix_hash: Some(header.mix_hash),
            nonce: Some(header.nonce),
            base_fee_per_gas: header.base_fee_per_gas,
            miner: Some(header.beneficiary),
            withdrawals: value
                .block
                .withdrawals()
                .map(<[edr_eth::withdrawal::Withdrawal]>::to_vec),
            withdrawals_root: header.withdrawals_root,
            blob_gas_used: header.blob_gas.as_ref().map(|bg| bg.gas_used),
            excess_blob_gas: header.blob_gas.as_ref().map(|bg| bg.excess_gas),
            parent_beacon_block_root: header.parent_beacon_block_root,
        }
    }
}

// ----------

// impl<TimerT: Clone + TimeSinceEpoch> FromRpcType<CallRequest, TimerT> for Request {
//     type Context<'context, ChainSpecT> = CallContext<'context, GenericChainSpec, TimerT>;

//     type Error = ProviderError<GenericChainSpec>;

//     fn from_rpc_type(value: CallRequest, context: Self::Context<'_>) -> Result<Self, Self::Error> {
//         let CallContext::<GenericChainSpec, TimerT> {
//             data,
//             block_spec,
//             state_overrides,
//             default_gas_price_fn,
//             max_fees_fn,
//         } = context;

//         validate_call_request::<GenericChainSpec>(data.evm_spec_id(), &value, block_spec)?;

//         let CallRequest {
//             from,
//             to,
//             gas,
//             gas_price,
//             max_fee_per_gas,
//             max_priority_fee_per_gas,
//             value,
//             data: input,
//             access_list,
//             ..
//         } = value;

//         let chain_id = data.chain_id();
//         let sender = from.unwrap_or_else(|| data.default_caller());
//         let gas_limit = gas.unwrap_or_else(|| data.block_gas_limit());
//         let input = input.map_or(Bytes::new(), Bytes::from);
//         let nonce = data.nonce(&sender, Some(block_spec), state_overrides)?;
//         let value = value.unwrap_or(U256::ZERO);

//         let evm_spec_id = data.evm_spec_id();
//         let request = if evm_spec_id < SpecId::LONDON || gas_price.is_some() {
//             let gas_price = gas_price.map_or_else(|| default_gas_price_fn(data), Ok)?;
//             match access_list {
//                 Some(access_list) if evm_spec_id >= SpecId::BERLIN => Request::Eip2930(Eip2930 {
//                     nonce,
//                     gas_price,
//                     gas_limit,
//                     value,
//                     input,
//                     kind: to.into(),
//                     chain_id,
//                     access_list,
//                 }),
//                 _ => Request::Eip155(Eip155 {
//                     nonce,
//                     gas_price,
//                     gas_limit,
//                     kind: to.into(),
//                     value,
//                     input,
//                     chain_id,
//                 }),
//             }
//         } else {
//             let (max_fee_per_gas, max_priority_fee_per_gas) =
//                 max_fees_fn(data, block_spec, max_fee_per_gas, max_priority_fee_per_gas)?;

//             Request::Eip1559(Eip1559 {
//                 chain_id,
//                 nonce,
//                 max_fee_per_gas,
//                 max_priority_fee_per_gas,
//                 gas_limit,
//                 kind: to.into(),
//                 value,
//                 input,
//                 access_list: access_list.unwrap_or_default(),
//             })
//         };

//         Result::<Self, ProviderError<GenericChainSpec>>::Ok(request)
//     }
// }

// impl<TimerT: Clone + TimeSinceEpoch> FromRpcType<TransactionRequest, TimerT> for Request {
//     type Context<'context> = TransactionContext<'context, GenericChainSpec, TimerT>;

//     type Error = ProviderError<GenericChainSpec>;

//     fn from_rpc_type(
//         value: TransactionRequest,
//         context: Self::Context<'_>,
//     ) -> Result<Request, ProviderError<GenericChainSpec>> {
//         const DEFAULT_MAX_PRIORITY_FEE_PER_GAS: u64 = 1_000_000_000;

//         /// # Panics
//         ///
//         /// Panics if `data.evm_spec_id()` is less than `SpecId::LONDON`.
//         fn calculate_max_fee_per_gas<TimerT: Clone + TimeSinceEpoch>(
//             data: &ProviderData<GenericChainSpec, TimerT>,
//             max_priority_fee_per_gas: U256,
//         ) -> Result<U256, BlockchainError<GenericChainSpec>> {
//             let base_fee_per_gas = data
//                 .next_block_base_fee_per_gas()?
//                 .expect("We already validated that the block is post-London.");
//             Ok(U256::from(2) * base_fee_per_gas + max_priority_fee_per_gas)
//         }

//         let TransactionContext::<GenericChainSpec, TimerT> { data } = context;

//         validate_send_transaction_request(data, &value)?;

//         let TransactionRequest {
//             from,
//             to,
//             gas_price,
//             max_fee_per_gas,
//             max_priority_fee_per_gas,
//             gas,
//             value,
//             data: input,
//             nonce,
//             chain_id,
//             access_list,
//             // We ignore the transaction type
//             transaction_type: _transaction_type,
//             blobs: _blobs,
//             blob_hashes: _blob_hashes,
//         } = value;

//         let chain_id = chain_id.unwrap_or_else(|| data.chain_id());
//         let gas_limit = gas.unwrap_or_else(|| data.block_gas_limit());
//         let input = input.map_or(Bytes::new(), Into::into);
//         let nonce = nonce.map_or_else(|| data.account_next_nonce(&from), Ok)?;
//         let value = value.unwrap_or(U256::ZERO);

//         let request = match (
//             gas_price,
//             max_fee_per_gas,
//             max_priority_fee_per_gas,
//             access_list,
//         ) {
//             (gas_price, max_fee_per_gas, max_priority_fee_per_gas, access_list)
//                 if data.evm_spec_id() >= SpecId::LONDON
//                     && (gas_price.is_none()
//                         || max_fee_per_gas.is_some()
//                         || max_priority_fee_per_gas.is_some()) =>
//             {
//                 let (max_fee_per_gas, max_priority_fee_per_gas) =
//                     match (max_fee_per_gas, max_priority_fee_per_gas) {
//                         (Some(max_fee_per_gas), Some(max_priority_fee_per_gas)) => {
//                             (max_fee_per_gas, max_priority_fee_per_gas)
//                         }
//                         (Some(max_fee_per_gas), None) => (
//                             max_fee_per_gas,
//                             max_fee_per_gas.min(U256::from(DEFAULT_MAX_PRIORITY_FEE_PER_GAS)),
//                         ),
//                         (None, Some(max_priority_fee_per_gas)) => {
//                             let max_fee_per_gas =
//                                 calculate_max_fee_per_gas(data, max_priority_fee_per_gas)?;
//                             (max_fee_per_gas, max_priority_fee_per_gas)
//                         }
//                         (None, None) => {
//                             let max_priority_fee_per_gas =
//                                 U256::from(DEFAULT_MAX_PRIORITY_FEE_PER_GAS);
//                             let max_fee_per_gas =
//                                 calculate_max_fee_per_gas(data, max_priority_fee_per_gas)?;
//                             (max_fee_per_gas, max_priority_fee_per_gas)
//                         }
//                     };

//                 Request::Eip1559(Eip1559 {
//                     nonce,
//                     max_priority_fee_per_gas,
//                     max_fee_per_gas,
//                     gas_limit,
//                     value,
//                     input,
//                     kind: to.into(),
//                     chain_id,
//                     access_list: access_list.unwrap_or_default(),
//                 })
//             }
//             (gas_price, _, _, Some(access_list)) => Request::Eip2930(Eip2930 {
//                 nonce,
//                 gas_price: gas_price.map_or_else(|| data.next_gas_price(), Ok)?,
//                 gas_limit,
//                 value,
//                 input,
//                 kind: to.into(),
//                 chain_id,
//                 access_list,
//             }),
//             (gas_price, _, _, _) => Request::Eip155(Eip155 {
//                 nonce,
//                 gas_price: gas_price.map_or_else(|| data.next_gas_price(), Ok)?,
//                 gas_limit,
//                 value,
//                 input,
//                 kind: to.into(),
//                 chain_id,
//             }),
//         };

//         Ok(request)
//     }
// }
