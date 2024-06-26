use core::fmt::Debug;
use std::sync::Arc;

use edr_eth::{
    receipt::{BlockReceipt, TransactionReceipt},
    rlp::Decodable,
    transaction::{
        pooled::PooledTransaction, request::TransactionRequestAndSender, EthTransactionRequest,
        SignedTransaction, Transaction as _, TxKind,
    },
    Bytes, PreEip1898BlockSpec, SpecId, B256, U256,
};
use edr_evm::{
    blockchain::BlockchainError, chain_spec::L1ChainSpec, trace::Trace, transaction, SyncBlock,
};

use crate::{
    data::{BlockDataForTransaction, ProviderData, TransactionAndBlock},
    error::TransactionFailureWithTraces,
    requests::validation::{
        validate_eip3860_max_initcode_size, validate_post_merge_block_tags,
        validate_transaction_and_call_request,
    },
    time::TimeSinceEpoch,
    ProviderError, TransactionFailure,
};

const FIRST_HARDFORK_WITH_TRANSACTION_TYPE: SpecId = SpecId::BERLIN;

pub fn handle_get_transaction_by_block_hash_and_index<
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<LoggerErrorT, TimerT>,
    block_hash: B256,
    index: U256,
) -> Result<Option<edr_rpc_eth::Transaction>, ProviderError<LoggerErrorT>> {
    let index = rpc_index_to_usize(&index)?;

    data.block_by_hash(&block_hash)?
        .and_then(|block| transaction_from_block(block, index, false))
        .map(|tx| transaction_to_rpc_result(tx, data.spec_id()))
        .transpose()
}

pub fn handle_get_transaction_by_block_spec_and_index<
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    block_spec: PreEip1898BlockSpec,
    index: U256,
) -> Result<Option<edr_rpc_eth::Transaction>, ProviderError<LoggerErrorT>> {
    validate_post_merge_block_tags(data.spec_id(), &block_spec)?;

    let index = rpc_index_to_usize(&index)?;

    match data.block_by_block_spec(&block_spec.into()) {
        Ok(Some(block)) => Some((block, false)),
        // Pending block requested
        Ok(None) => {
            let result = data.mine_pending_block()?;
            let block: Arc<dyn SyncBlock<L1ChainSpec, Error = BlockchainError<L1ChainSpec>>> =
                Arc::new(result.block);
            Some((block, true))
        }
        // Matching Hardhat behavior in returning None for invalid block hash or number.
        Err(ProviderError::InvalidBlockNumberOrHash { .. }) => None,
        Err(err) => return Err(err),
    }
    .and_then(|(block, is_pending)| transaction_from_block(block, index, is_pending))
    .map(|tx| transaction_to_rpc_result(tx, data.spec_id()))
    .transpose()
}

pub fn handle_pending_transactions<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<LoggerErrorT, TimerT>,
) -> Result<Vec<edr_rpc_eth::Transaction>, ProviderError<LoggerErrorT>> {
    let spec_id = data.spec_id();
    data.pending_transactions()
        .map(|pending_transaction| {
            let transaction_and_block = TransactionAndBlock {
                transaction: pending_transaction.clone(),
                block_data: None,
                is_pending: true,
            };
            transaction_to_rpc_result(transaction_and_block, spec_id)
        })
        .collect()
}

fn rpc_index_to_usize<LoggerErrorT: Debug>(
    index: &U256,
) -> Result<usize, ProviderError<LoggerErrorT>> {
    index
        .try_into()
        .map_err(|_err| ProviderError::InvalidTransactionIndex(*index))
}

pub fn handle_get_transaction_by_hash<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<LoggerErrorT, TimerT>,
    transaction_hash: B256,
) -> Result<Option<edr_rpc_eth::Transaction>, ProviderError<LoggerErrorT>> {
    data.transaction_by_hash(&transaction_hash)?
        .map(|tx| transaction_to_rpc_result(tx, data.spec_id()))
        .transpose()
}

pub fn handle_get_transaction_receipt<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<LoggerErrorT, TimerT>,
    transaction_hash: B256,
) -> Result<Option<Arc<BlockReceipt>>, ProviderError<LoggerErrorT>> {
    let receipt = data.transaction_receipt(&transaction_hash)?;

    // The JSON-RPC layer should not return the gas price as effective gas price for
    // receipts in pre-London hardforks.
    if let Some(receipt) = receipt.as_ref() {
        if data.spec_id() < SpecId::LONDON && receipt.effective_gas_price.is_some() {
            return Ok(Some(Arc::new(BlockReceipt {
                inner: TransactionReceipt {
                    effective_gas_price: None,
                    ..receipt.inner.clone()
                },
                block_hash: receipt.block_hash,
                block_number: receipt.block_number,
            })));
        }
    }

    Ok(receipt)
}

fn transaction_from_block(
    block: Arc<dyn SyncBlock<L1ChainSpec, Error = BlockchainError<L1ChainSpec>>>,
    transaction_index: usize,
    is_pending: bool,
) -> Option<TransactionAndBlock> {
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

pub fn transaction_to_rpc_result<LoggerErrorT: Debug>(
    transaction_and_block: TransactionAndBlock,
    spec_id: SpecId,
) -> Result<edr_rpc_eth::Transaction, ProviderError<LoggerErrorT>> {
    fn gas_price_for_post_eip1559(
        signed_transaction: &transaction::Signed,
        block: Option<&Arc<dyn SyncBlock<L1ChainSpec, Error = BlockchainError<L1ChainSpec>>>>,
    ) -> U256 {
        let max_fee_per_gas = signed_transaction
            .max_fee_per_gas()
            .expect("Transaction must be post EIP-1559 transaction.");
        let max_priority_fee_per_gas = *signed_transaction
            .max_priority_fee_per_gas()
            .expect("Transaction must be post EIP-1559 transaction.");

        if let Some(block) = block {
            let base_fee_per_gas = block.header().base_fee_per_gas.expect(
                "Transaction must have base fee per gas in block metadata if EIP-1559 is active.",
            );
            let priority_fee_per_gas =
                max_priority_fee_per_gas.min(max_fee_per_gas - base_fee_per_gas);
            base_fee_per_gas + priority_fee_per_gas
        } else {
            // We are following Hardhat's behavior of returning the max fee per gas for
            // pending transactions.
            max_fee_per_gas
        }
    }

    let TransactionAndBlock {
        transaction,
        block_data,
        is_pending,
    } = transaction_and_block;
    let block = block_data.as_ref().map(|b| &b.block);
    let header = block.map(|b| b.header());

    let gas_price = match &transaction {
        transaction::Signed::PreEip155Legacy(tx) => tx.gas_price,
        transaction::Signed::PostEip155Legacy(tx) => tx.gas_price,
        transaction::Signed::Eip2930(tx) => tx.gas_price,
        transaction::Signed::Eip1559(_) | transaction::Signed::Eip4844(_) => {
            gas_price_for_post_eip1559(&transaction, block)
        }
    };

    let chain_id = match &transaction {
        // Following Hardhat in not returning `chain_id` for `PostEip155Legacy` legacy transactions
        // even though the chain id would be recoverable.
        transaction::Signed::PreEip155Legacy(_) | transaction::Signed::PostEip155Legacy(_) => None,
        transaction::Signed::Eip2930(tx) => Some(tx.chain_id),
        transaction::Signed::Eip1559(tx) => Some(tx.chain_id),
        transaction::Signed::Eip4844(tx) => Some(tx.chain_id),
    };

    let show_transaction_type = spec_id >= FIRST_HARDFORK_WITH_TRANSACTION_TYPE;
    let is_typed_transaction = transaction.transaction_type() > transaction::Type::Legacy;
    let transaction_type = if show_transaction_type || is_typed_transaction {
        Some(transaction.transaction_type())
    } else {
        None
    };

    let signature = transaction.signature();
    let (block_hash, block_number) = if is_pending {
        (None, None)
    } else {
        header
            .map(|header| (header.hash(), U256::from(header.number)))
            .unzip()
    };

    let transaction_index = if is_pending {
        None
    } else {
        block_data.as_ref().map(|bd| bd.transaction_index)
    };

    let access_list = if transaction.transaction_type() >= transaction::Type::Eip2930 {
        Some(transaction.access_list().to_vec())
    } else {
        None
    };

    let blob_versioned_hashes = if transaction.transaction_type() == transaction::Type::Eip4844 {
        Some(transaction.blob_hashes().to_vec())
    } else {
        None
    };

    Ok(edr_rpc_eth::Transaction {
        hash: *transaction.transaction_hash(),
        nonce: transaction.nonce(),
        block_hash,
        block_number,
        transaction_index,
        from: *transaction.caller(),
        to: transaction.kind().to().copied(),
        value: *transaction.value(),
        gas_price,
        gas: U256::from(transaction.gas_limit()),
        input: transaction.data().clone(),
        v: signature.v(),
        // Following Hardhat in always returning `v` instead of `y_parity`.
        y_parity: None,
        r: signature.r(),
        s: signature.s(),
        chain_id,
        transaction_type: transaction_type.map(u8::from),
        access_list,
        max_fee_per_gas: transaction.max_fee_per_gas(),
        max_priority_fee_per_gas: transaction.max_priority_fee_per_gas().cloned(),
        max_fee_per_blob_gas: transaction.max_fee_per_blob_gas().cloned(),
        blob_versioned_hashes,
    })
}

pub fn handle_send_transaction_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    transaction_request: EthTransactionRequest,
) -> Result<(B256, Vec<Trace<L1ChainSpec>>), ProviderError<LoggerErrorT>> {
    validate_send_transaction_request(data, &transaction_request)?;

    let transaction_request = resolve_transaction_request(data, transaction_request)?;
    let signed_transaction = data.sign_transaction_request(transaction_request)?;

    send_raw_transaction_and_log(data, signed_transaction)
}

pub fn handle_send_raw_transaction_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    raw_transaction: Bytes,
) -> Result<(B256, Vec<Trace<L1ChainSpec>>), ProviderError<LoggerErrorT>> {
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

    let signed_transaction = transaction::validate(signed_transaction, data.spec_id())
        .map_err(ProviderError::TransactionCreationError)?;

    send_raw_transaction_and_log(data, signed_transaction)
}

fn resolve_transaction_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    transaction_request: EthTransactionRequest,
) -> Result<TransactionRequestAndSender, ProviderError<LoggerErrorT>> {
    const DEFAULT_MAX_PRIORITY_FEE_PER_GAS: u64 = 1_000_000_000;

    /// # Panics
    ///
    /// Panics if `data.spec_id()` is less than `SpecId::LONDON`.
    fn calculate_max_fee_per_gas<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
        data: &ProviderData<LoggerErrorT, TimerT>,
        max_priority_fee_per_gas: U256,
    ) -> Result<U256, BlockchainError<L1ChainSpec>> {
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
            if data.spec_id() >= SpecId::LONDON
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

fn send_raw_transaction_and_log<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    signed_transaction: transaction::Signed,
) -> Result<(B256, Vec<Trace<L1ChainSpec>>), ProviderError<LoggerErrorT>> {
    let result = data.send_transaction(signed_transaction.clone())?;

    let spec_id = data.spec_id();
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

fn validate_send_transaction_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<LoggerErrorT, TimerT>,
    request: &EthTransactionRequest,
) -> Result<(), ProviderError<LoggerErrorT>> {
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
            data.spec_id(),
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

    validate_transaction_and_call_request(data.spec_id(), request).map_err(|err| match err {
        ProviderError::UnsupportedEIP1559Parameters {
            minimum_hardfork, ..
        } => ProviderError::InvalidArgument(format!("\
EIP-1559 style fee params (maxFeePerGas or maxPriorityFeePerGas) received but they are not supported by the current hardfork.

You can use them by running Hardhat Network with 'hardfork' {minimum_hardfork:?} or later.
        ")),
        err => err,
    })
}

fn validate_send_raw_transaction_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<LoggerErrorT, TimerT>,
    transaction: &transaction::Signed,
) -> Result<(), ProviderError<LoggerErrorT>> {
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
        data.spec_id(),
        data.allow_unlimited_initcode_size(),
        transaction.kind().to(),
        transaction.data(),
    )?;

    validate_transaction_and_call_request(data.spec_id(), transaction).map_err(|err| match err {
        ProviderError::UnsupportedEIP1559Parameters {
            minimum_hardfork, ..
        } => ProviderError::InvalidArgument(format!(
            "\
Trying to send an EIP-1559 transaction but they are not supported by the current hard fork.\
\
You can use them by running Hardhat Network with 'hardfork' {minimum_hardfork:?} or later."
        )),
        err => err,
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use edr_eth::{Address, Bytes, U256};

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
        let transaction = transaction::validate(transaction, fixture.provider_data.spec_id())?;

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
