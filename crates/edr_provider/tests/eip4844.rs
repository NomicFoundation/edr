#![cfg(feature = "test-utils")]

use std::str::FromStr;

use edr_eth::{
    remote::{self, PreEip1898BlockSpec},
    rlp::{self, Decodable},
    transaction::{
        pooled::{eip4844::BYTES_PER_BLOB, PooledTransaction},
        EthTransactionRequest, SignedTransaction, Transaction,
    },
    AccountInfo, Address, Bytes, B256,
};
use edr_evm::{address, ExecutableTransaction, KECCAK_EMPTY};
use edr_provider::{
    test_utils::{create_test_config, one_ether},
    MethodInvocation, NoopLogger, Provider, ProviderError, ProviderRequest,
};
use tokio::runtime;

const CALLER: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

/// Must match the value in `fixtures/eip4844.txt`.
fn fake_blobs() -> Vec<Bytes> {
    const BLOB_VALUE: &[u8] = b"hello world";

    // The blob starts 0, followed by `hello world`, then 0x80, and is padded with
    // zeroes.
    let mut bytes = vec![0x0u8];
    bytes.append(&mut BLOB_VALUE.to_vec());
    bytes.push(0x80u8);

    bytes.resize(BYTES_PER_BLOB, 0);

    // let blob = c_kzg::Blob::from_bytes(bytes.as_slice()).expect("Invalid blob")

    vec![Bytes::from(bytes)]
}

fn fake_raw_transaction() -> Bytes {
    Bytes::from_str(include_str!("fixtures/eip4844.txt")).expect("failed to parse raw transaction")
}

fn fake_raw_transaction_with_four_blobs() -> Bytes {
    todo!("Add file that contains an RLP-encoded transaction with 4 blobs")
}

fn fake_pooled_transaction() -> PooledTransaction {
    let raw_transaction = fake_raw_transaction();

    PooledTransaction::decode(&mut raw_transaction.as_ref())
        .expect("failed to decode raw transaction")
}

fn fake_transaction() -> SignedTransaction {
    fake_pooled_transaction().into_payload()
}

fn fake_transaction_request() -> anyhow::Result<EthTransactionRequest> {
    let transaction = fake_transaction();
    let from = transaction.recover()?;

    Ok(EthTransactionRequest {
        from,
        to: transaction.to(),
        max_fee_per_gas: transaction.max_fee_per_gas(),
        max_priority_fee_per_gas: transaction.max_priority_fee_per_gas(),
        gas: Some(transaction.gas_limit()),
        value: Some(transaction.value()),
        data: Some(transaction.data().clone()),
        nonce: Some(transaction.nonce()),
        chain_id: transaction.chain_id(),
        access_list: transaction
            .access_list()
            .map(|access_list| access_list.0.clone()),
        transaction_type: Some(transaction.transaction_type().into()),
        blobs: Some(fake_blobs()),
        blob_hashes: transaction.blob_hashes(),
        ..EthTransactionRequest::default()
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn send_transaction_unsupported() -> anyhow::Result<()> {
    let transaction = fake_transaction_request()?;

    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = transaction.chain_id.expect("Blob transaction has chain ID");

    let provider = Provider::new(runtime::Handle::current(), logger, subscriber, config)?;

    let error = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::SendTransaction(
            transaction,
        )))
        .expect_err("Must return an error");

    assert!(matches!(
        error,
        ProviderError::Eip4844TransactionUnsupported
    ));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_raw_transaction() -> anyhow::Result<()> {
    let raw_eip4844_transaction = fake_raw_transaction();

    let expected = fake_transaction();

    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = expected.chain_id().expect("Blob transaction has chain ID");

    config.genesis_accounts.insert(
        CALLER,
        AccountInfo {
            balance: one_ether(),
            nonce: 0,
            code: None,
            code_hash: KECCAK_EMPTY,
        },
    );

    let provider = Provider::new(runtime::Handle::current(), logger, subscriber, config)?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendRawTransaction(raw_eip4844_transaction),
    ))?;

    let transaction_hash: B256 = serde_json::from_value(result.result)?;
    assert_eq!(transaction_hash, *expected.transaction_hash());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_transaction() -> anyhow::Result<()> {
    let raw_eip4844_transaction = fake_raw_transaction();

    let expected = fake_transaction();

    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = expected.chain_id().expect("Blob transaction has chain ID");

    config.genesis_accounts.insert(
        CALLER,
        AccountInfo {
            balance: one_ether(),
            nonce: 0,
            code: None,
            code_hash: KECCAK_EMPTY,
        },
    );

    let provider = Provider::new(runtime::Handle::current(), logger, subscriber, config)?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendRawTransaction(raw_eip4844_transaction),
    ))?;

    let transaction_hash: B256 = serde_json::from_value(result.result)?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetTransactionByHash(transaction_hash),
    ))?;

    let transaction: remote::eth::Transaction = serde_json::from_value(result.result)?;
    let transaction = ExecutableTransaction::try_from(transaction)?;

    assert_eq!(transaction.into_inner().0, expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn block_header() -> anyhow::Result<()> {
    let raw_eip4844_transaction = fake_raw_transaction();

    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = fake_transaction()
        .chain_id()
        .expect("Blob transaction has chain ID");

    config.genesis_accounts.insert(
        CALLER,
        AccountInfo {
            balance: one_ether(),
            nonce: 0,
            code: None,
            code_hash: KECCAK_EMPTY,
        },
    );

    let provider = Provider::new(runtime::Handle::current(), logger, subscriber, config)?;

    // The genesis block has 0 excess blobs
    let mut excess_blobs = 0u64;

    provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendRawTransaction(raw_eip4844_transaction),
    ))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let first_block: remote::eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(first_block.blob_gas_used, Some(BYTES_PER_BLOB as u64));

    assert_eq!(
        first_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    // The first block does not affect the number of excess blobs, as it has less
    // than the target number of blobs (3)

    provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendRawTransaction(fake_raw_transaction_with_four_blobs()),
    ))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let second_block: remote::eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(second_block.blob_gas_used, Some(4 * BYTES_PER_BLOB as u64));

    assert_eq!(
        second_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    // The second block increases the excess by 1 blob (4 - 3)
    excess_blobs += 1;

    provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendRawTransaction(fake_raw_transaction_with_four_blobs()),
    ))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let third_block: remote::eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(third_block.blob_gas_used, Some(4 * BYTES_PER_BLOB as u64));

    assert_eq!(
        third_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    // The third block increases the excess by 1 blob (4 - 3)
    excess_blobs += 1;

    // Mine an empty block to validate the previous block's excess
    provider.handle_request(ProviderRequest::Single(MethodInvocation::Mine(None, None)))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let fourth_block: remote::eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(fourth_block.blob_gas_used, Some(0u64));

    assert_eq!(
        fourth_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    // The fourth block decreases the excess by 3 blob (0 - 3), but should not go
    // below 0 - the minimum
    excess_blobs = excess_blobs.saturating_sub(3);

    // Mine an empty block to validate the previous block's excess
    provider.handle_request(ProviderRequest::Single(MethodInvocation::Mine(None, None)))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let fifth_block: remote::eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(fifth_block.blob_gas_used, Some(0u64));

    assert_eq!(
        fifth_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    Ok(())
}
