use std::sync::Arc;

use edr_eth::{eips::eip4844::BYTES_PER_BLOB, PreEip1898BlockSpec, SpecId, B256};
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

use crate::common::blob::{fake_raw_transaction, fake_transaction, BlobTransactionBuilder};

#[tokio::test(flavor = "multi_thread")]
async fn block_header() -> anyhow::Result<()> {
    let raw_eip4844_transaction = fake_raw_transaction();

    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = fake_transaction()
        .chain_id()
        .expect("Blob transaction has chain ID");
    config.hardfork = SpecId::PRAGUE;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    // The genesis block has 0 excess blobs
    let mut excess_blobs = 0u64;

    provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendRawTransaction(raw_eip4844_transaction),
    ))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let first_block: edr_rpc_eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(first_block.blob_gas_used, Some(BYTES_PER_BLOB as u64));

    assert_eq!(
        first_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    // The first block does not affect the number of excess blobs, as it has less
    // than the target number of blobs (6)
    let excess_blob_transaction = BlobTransactionBuilder::default()
        .duplicate_blobs(7)
        .nonce(1)
        .build_raw();

    provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendRawTransaction(excess_blob_transaction),
    ))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let second_block: edr_rpc_eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(second_block.blob_gas_used, Some(7 * BYTES_PER_BLOB as u64));

    assert_eq!(
        second_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    // The second block increases the excess by 1 blob (7 - 6)
    excess_blobs += 1;

    let excess_blob_transaction = BlobTransactionBuilder::default()
        .duplicate_blobs(8)
        .nonce(2)
        .build_raw();

    provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendRawTransaction(excess_blob_transaction),
    ))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let third_block: edr_rpc_eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(third_block.blob_gas_used, Some(8 * BYTES_PER_BLOB as u64));

    assert_eq!(
        third_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    // The third block increases the excess by 2 blob (8 - 6)
    excess_blobs += 2;

    // Mine an empty block to validate the previous block's excess
    provider.handle_request(ProviderRequest::Single(MethodInvocation::Mine(None, None)))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let fourth_block: edr_rpc_eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(fourth_block.blob_gas_used, Some(0u64));

    assert_eq!(
        fourth_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    // The fourth block decreases the excess by 6 blob (0 - 6), but should not go
    // below 0 - the minimum
    excess_blobs = excess_blobs.saturating_sub(6);

    // Mine an empty block to validate the previous block's excess
    provider.handle_request(ProviderRequest::Single(MethodInvocation::Mine(None, None)))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let fifth_block: edr_rpc_eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(fifth_block.blob_gas_used, Some(0u64));

    assert_eq!(
        fifth_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    Ok(())
}
