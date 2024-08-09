use core::fmt::Debug;
use std::sync::Arc;

use edr_eth::{
    result::InvalidTransaction,
    rlp::Decodable,
    transaction::{
        pooled::PooledTransaction, request::TransactionRequestAndSender, EthTransactionRequest,
        IsEip4844, Transaction as _, TransactionType, TransactionValidation, TxKind,
    },
    Bytes, PreEip1898BlockSpec, SpecId, B256, U256,
};
use edr_evm::{
    block::transaction::{BlockDataForTransaction, TransactionAndBlock},
    blockchain::BlockchainError,
    chain_spec::ChainSpec,
    trace::Trace,
    transaction, SyncBlock,
};
use edr_rpc_eth::RpcTypeFrom as _;

use crate::{
    data::ProviderData,
    error::TransactionFailureWithTraces,
    requests::validation::{
        validate_eip3860_max_initcode_size, validate_post_merge_block_tags,
        validate_transaction_and_call_request,
    },
    spec::SyncProviderSpec,
    time::TimeSinceEpoch,
    ProviderError, TransactionFailure, TransactionFailureReason,
};

pub fn handle_get_transaction_by_block_hash_and_index<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
    block_hash: B256,
    index: U256,
) -> Result<Option<ChainSpecT::RpcTransaction>, ProviderError<ChainSpecT>> {
    let index = rpc_index_to_usize(&index)?;

    let transaction = data
        .block_by_hash(&block_hash)?
        .and_then(|block| transaction_from_block(block, index, false))
        .map(|transaction_and_block| {
            ChainSpecT::RpcTransaction::rpc_type_from(&transaction_and_block, data.hardfork())
        });

    Ok(transaction)
}

pub fn handle_get_transaction_by_block_spec_and_index<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        Block: Default,
        Transaction: Default
                         + TransactionValidation<
            ValidationError: From<InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    block_spec: PreEip1898BlockSpec,
    index: U256,
) -> Result<Option<ChainSpecT::RpcTransaction>, ProviderError<ChainSpecT>> {
    validate_post_merge_block_tags(data.evm_spec_id(), &block_spec)?;

    let index = rpc_index_to_usize(&index)?;

    let transaction = match data.block_by_block_spec(&block_spec.into()) {
        Ok(Some(block)) => Some((block, false)),
        // Pending block requested
        Ok(None) => {
            let result = data.mine_pending_block()?;
            let block: Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainError<ChainSpecT>>> =
                Arc::new(result.block);
            Some((block, true))
        }
        // Matching Hardhat behavior in returning None for invalid block hash or number.
        Err(ProviderError::InvalidBlockNumberOrHash { .. }) => None,
        Err(err) => return Err(err),
    }
    .and_then(|(block, is_pending)| transaction_from_block(block, index, is_pending))
    .map(|transaction_and_block| {
        ChainSpecT::RpcTransaction::rpc_type_from(&transaction_and_block, data.hardfork())
    });

    Ok(transaction)
}

pub fn handle_pending_transactions<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
) -> Result<Vec<ChainSpecT::RpcTransaction>, ProviderError<ChainSpecT>> {
    let transactions = data
        .pending_transactions()
        .map(|pending_transaction| {
            let transaction_and_block = TransactionAndBlock {
                transaction: pending_transaction.clone(),
                block_data: None,
                is_pending: true,
            };
            ChainSpecT::RpcTransaction::rpc_type_from(&transaction_and_block, data.hardfork())
        })
        .collect();

    Ok(transactions)
}

fn rpc_index_to_usize<ChainSpecT: ChainSpec<Hardfork: Debug>>(
    index: &U256,
) -> Result<usize, ProviderError<ChainSpecT>> {
    index
        .try_into()
        .map_err(|_err| ProviderError::InvalidTransactionIndex(*index))
}

pub fn handle_get_transaction_by_hash<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
    transaction_hash: B256,
) -> Result<Option<ChainSpecT::RpcTransaction>, ProviderError<ChainSpecT>> {
    let transaction = data
        .transaction_by_hash(&transaction_hash)?
        .map(|transaction_and_block| {
            ChainSpecT::RpcTransaction::rpc_type_from(&transaction_and_block, data.hardfork())
        });

    Ok(transaction)
}

pub fn handle_get_transaction_receipt<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
    transaction_hash: B256,
) -> Result<Option<ChainSpecT::RpcReceipt>, ProviderError<ChainSpecT>> {
    let receipt = data.transaction_receipt(&transaction_hash)?;

    Ok(receipt.map(|receipt| ChainSpecT::RpcReceipt::rpc_type_from(&receipt, data.hardfork())))
}

fn transaction_from_block<ChainSpecT: ChainSpec>(
    block: Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainError<ChainSpecT>>>,
    transaction_index: usize,
    is_pending: bool,
) -> Option<TransactionAndBlock<ChainSpecT>> {
    block
        .transactions()
        .get(transaction_index)
        .map(|transaction| TransactionAndBlock {
            transaction: transaction.clone(),
            block_data: Some(BlockDataForTransaction {
                block: block.clone(),
                transaction_index: transaction_index.try_into().expect("usize fits into u64"),
            }),
            is_pending,
        })
}

pub fn handle_send_transaction_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        Block: Default,
        HaltReason: Into<TransactionFailureReason<ChainSpecT>>,
        Transaction: Default
                         + TransactionType<Type: IsEip4844>
                         + TransactionValidation<
            ValidationError: From<InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    transaction_request: EthTransactionRequest,
) -> Result<(B256, Vec<Trace<ChainSpecT>>), ProviderError<ChainSpecT>> {
    validate_send_transaction_request(data, &transaction_request)?;

    let transaction_request = resolve_transaction_request(data, transaction_request)?;
    let signed_transaction = data.sign_transaction_request(transaction_request)?;

    send_raw_transaction_and_log(data, signed_transaction)
}

pub fn handle_send_raw_transaction_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        Block: Default,
        HaltReason: Into<TransactionFailureReason<ChainSpecT>>,
        Transaction: Default
                         + TransactionType<Type: IsEip4844>
                         + TransactionValidation<
            ValidationError: From<InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    raw_transaction: Bytes,
) -> Result<(B256, Vec<Trace<ChainSpecT>>), ProviderError<ChainSpecT>> {
    let mut raw_transaction: &[u8] = raw_transaction.as_ref();
    let pooled_transaction =
    PooledTransaction::decode(&mut raw_transaction).map_err(|err| match err {
            edr_eth::rlp::Error::Custom(message) if transaction::Signed::is_invalid_transaction_type_error(message) => {
                let type_id = *raw_transaction.first().expect("We already validated that the transaction is not empty if it's an invalid transaction type error.");
                ProviderError::InvalidTransactionType(type_id)
            }
            err => ProviderError::InvalidArgument(err.to_string()),
        })?;

    let signed_transaction = pooled_transaction.into_payload();
    validate_send_raw_transaction_request(data, &signed_transaction)?;

    let signed_transaction = transaction::validate(signed_transaction, data.evm_spec_id())
        .map_err(ProviderError::TransactionCreationError)?;

    send_raw_transaction_and_log(data, signed_transaction)
}

fn resolve_transaction_request<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    transaction_request: EthTransactionRequest,
) -> Result<TransactionRequestAndSender<ChainSpecT::TransactionRequest>, ProviderError<ChainSpecT>>
{
    const DEFAULT_MAX_PRIORITY_FEE_PER_GAS: u64 = 1_000_000_000;

    /// # Panics
    ///
    /// Panics if `data.evm_spec_id()` is less than `SpecId::LONDON`.
    fn calculate_max_fee_per_gas<
        ChainSpecT: SyncProviderSpec<TimerT>,
        TimerT: Clone + TimeSinceEpoch,
    >(
        data: &ProviderData<ChainSpecT, TimerT>,
        max_priority_fee_per_gas: U256,
    ) -> Result<U256, BlockchainError<ChainSpecT>> {
        let base_fee_per_gas = data
            .next_block_base_fee_per_gas()?
            .expect("We already validated that the block is post-London.");
        Ok(U256::from(2) * base_fee_per_gas + max_priority_fee_per_gas)
    }

    let EthTransactionRequest {
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
    } = transaction_request;

    let chain_id = chain_id.unwrap_or_else(|| data.chain_id());
    let gas_limit = gas.unwrap_or_else(|| data.block_gas_limit());
    let input = input.map_or(Bytes::new(), Into::into);
    let nonce = nonce.map_or_else(|| data.account_next_nonce(&from), Ok)?;
    let value = value.unwrap_or(U256::ZERO);

    let request = match (
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        access_list,
    ) {
        (gas_price, max_fee_per_gas, max_priority_fee_per_gas, access_list)
            if data.evm_spec_id() >= SpecId::LONDON
                && (gas_price.is_none()
                    || max_fee_per_gas.is_some()
                    || max_priority_fee_per_gas.is_some()) =>
        {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                match (max_fee_per_gas, max_priority_fee_per_gas) {
                    (Some(max_fee_per_gas), Some(max_priority_fee_per_gas)) => {
                        (max_fee_per_gas, max_priority_fee_per_gas)
                    }
                    (Some(max_fee_per_gas), None) => (
                        max_fee_per_gas,
                        max_fee_per_gas.min(U256::from(DEFAULT_MAX_PRIORITY_FEE_PER_GAS)),
                    ),
                    (None, Some(max_priority_fee_per_gas)) => {
                        let max_fee_per_gas =
                            calculate_max_fee_per_gas(data, max_priority_fee_per_gas)?;
                        (max_fee_per_gas, max_priority_fee_per_gas)
                    }
                    (None, None) => {
                        let max_priority_fee_per_gas = U256::from(DEFAULT_MAX_PRIORITY_FEE_PER_GAS);
                        let max_fee_per_gas =
                            calculate_max_fee_per_gas(data, max_priority_fee_per_gas)?;
                        (max_fee_per_gas, max_priority_fee_per_gas)
                    }
                };

            transaction::Request::Eip1559(transaction::request::Eip1559 {
                nonce,
                max_priority_fee_per_gas,
                max_fee_per_gas,
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
        }
        (gas_price, _, _, Some(access_list)) => {
            transaction::Request::Eip2930(transaction::request::Eip2930 {
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
        }
        (gas_price, _, _, _) => transaction::Request::Eip155(transaction::request::Eip155 {
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
        }),
    };

    Ok(TransactionRequestAndSender {
        request,
        sender: from,
    })
}

fn send_raw_transaction_and_log<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        Block: Default,
        HaltReason: Into<TransactionFailureReason<ChainSpecT>>,
        Transaction: Default
                         + TransactionType<Type: IsEip4844>
                         + TransactionValidation<
            ValidationError: From<InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    signed_transaction: ChainSpecT::Transaction,
) -> Result<(B256, Vec<Trace<ChainSpecT>>), ProviderError<ChainSpecT>> {
    let result = data.send_transaction(signed_transaction.clone())?;

    let spec_id = data.evm_spec_id();
    data.logger_mut()
        .log_send_transaction(spec_id, &signed_transaction, &result.mining_results)
        .map_err(ProviderError::Logger)?;

    if data.bail_on_transaction_failure() {
        let transaction_failure =
            result
                .transaction_result_and_trace()
                .and_then(|(execution_result, trace)| {
                    TransactionFailure::from_execution_result(
                        execution_result,
                        Some(&result.transaction_hash),
                        trace,
                    )
                });

        if let Some(failure) = transaction_failure {
            let (_transaction_hash, traces) = result.into();
            return Err(ProviderError::TransactionFailed(
                TransactionFailureWithTraces { failure, traces },
            ));
        }
    }

    Ok(result.into())
}

fn validate_send_transaction_request<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
    request: &EthTransactionRequest,
) -> Result<(), ProviderError<ChainSpecT>> {
    if let Some(chain_id) = request.chain_id {
        let expected = data.chain_id();
        if chain_id != expected {
            return Err(ProviderError::InvalidChainId {
                expected,
                actual: chain_id,
            });
        }
    }

    if let Some(request_data) = &request.data {
        validate_eip3860_max_initcode_size(
            data.evm_spec_id(),
            data.allow_unlimited_initcode_size(),
            request.to.as_ref(),
            request_data,
        )?;
    }

    if request.blob_hashes.is_some() || request.blobs.is_some() {
        return Err(ProviderError::Eip4844TransactionUnsupported);
    }

    if let Some(transaction_type) = request.transaction_type {
        if transaction_type == u8::from(transaction::Type::Eip4844) {
            return Err(ProviderError::Eip4844TransactionUnsupported);
        }
    }

    validate_transaction_and_call_request(data.evm_spec_id(), request).map_err(|err| match err {
        ProviderError::UnsupportedEIP1559Parameters {
            minimum_hardfork, ..
        } => ProviderError::InvalidArgument(format!("\
EIP-1559 style fee params (maxFeePerGas or maxPriorityFeePerGas) received but they are not supported by the current hardfork.

You can use them by running Hardhat Network with 'hardfork' {minimum_hardfork:?} or later.
        ")),
        err => err,
    })
}

fn validate_send_raw_transaction_request<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
    transaction: &transaction::Signed,
) -> Result<(), ProviderError<ChainSpecT>> {
    if let Some(tx_chain_id) = transaction.chain_id() {
        let expected = data.chain_id();
        if tx_chain_id != expected {
            let error = if transaction.is_eip155() {
                ProviderError::InvalidEip155TransactionChainId
            } else {
                ProviderError::InvalidArgument(format!("Trying to send a raw transaction with an invalid chainId. The expected chainId is {expected}"))
            };
            return Err(error);
        }
    }

    validate_eip3860_max_initcode_size(
        data.evm_spec_id(),
        data.allow_unlimited_initcode_size(),
        transaction.kind().to(),
        transaction.data(),
    )?;

    validate_transaction_and_call_request(data.evm_spec_id(), transaction).map_err(
        |err| match err {
            ProviderError::UnsupportedEIP1559Parameters {
                minimum_hardfork, ..
            } => ProviderError::InvalidArgument(format!(
                "\
Trying to send an EIP-1559 transaction but they are not supported by the current hard fork.\
\
You can use them by running Hardhat Network with 'hardfork' {minimum_hardfork:?} or later."
            )),
            err => err,
        },
    )
}

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use edr_eth::{Address, Bytes, U256};
    use transaction::signed::FakeSign as _;

    use super::*;
    use crate::{data::test_utils::ProviderTestFixture, test_utils::one_ether};

    #[test]
    fn transaction_by_hash_for_impersonated_account() -> anyhow::Result<()> {
        let mut fixture = ProviderTestFixture::new_local()?;

        let impersonated_account: Address = "0x20620fa0ad46516e915029c94e3c87c9cd7861ff".parse()?;
        fixture
            .provider_data
            .impersonate_account(impersonated_account);

        fixture
            .provider_data
            .set_balance(impersonated_account, one_ether())?;

        let chain_id = fixture.provider_data.chain_id();

        let transaction = transaction::Request::Eip155(transaction::request::Eip155 {
            kind: TxKind::Call(Address::ZERO),
            gas_limit: 30_000,
            gas_price: U256::from(42_000_000_000_u64),
            value: U256::from(1),
            input: Bytes::default(),
            nonce: 0,
            chain_id,
        })
        .fake_sign(impersonated_account);
        let transaction = transaction::validate(transaction, fixture.provider_data.evm_spec_id())?;

        fixture.provider_data.set_auto_mining(true);
        let result = fixture.provider_data.send_transaction(transaction)?;
        assert!(result.transaction_result_and_trace().is_some());

        let rpc_transaction =
            handle_get_transaction_by_hash(&fixture.provider_data, result.transaction_hash)?
                .context("transaction not found")?;
        assert_eq!(&rpc_transaction.from, &impersonated_account);
        assert_eq!(&rpc_transaction.hash, &result.transaction_hash);

        Ok(())
    }
}
