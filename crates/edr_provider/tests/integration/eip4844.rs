#![cfg(feature = "test-utils")]

use std::{str::FromStr, sync::Arc};

use alloy_eips::eip4844::DATA_GAS_PER_BLOB;
use edr_chain_l1::{
    rpc::{
        block::L1RpcBlock, call::L1CallRequest, transaction::L1RpcTransactionWithSignature,
        TransactionRequest,
    },
    L1ChainSpec,
};
use edr_defaults::SECRET_KEYS;
use edr_eth::{Blob, PreEip1898BlockSpec};
use edr_chain_spec::ExecutableTransaction as _;
use edr_primitives::{Address, Bytes, B256, U256};
use edr_provider::{
    test_utils::{create_test_config, deploy_contract, one_ether},
    time::CurrentTime,
    AccountOverride, MethodInvocation, NoopLogger, Provider, ProviderError, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::secret_key::secret_key_to_address;
use edr_transaction::TransactionType as _;
use tokio::runtime;

use crate::common::blob::{
    fake_pooled_transaction, fake_raw_transaction, fake_transaction, BlobTransactionBuilder,
};

fn fake_call_request() -> L1CallRequest {
    let transaction = fake_pooled_transaction();
    let blobs = transaction.blobs().map(<[Blob]>::to_vec);
    let transaction = transaction.into_payload();
    let from = transaction.caller();

    let blob_hashes = if transaction.transaction_type() == edr_chain_l1::L1TransactionType::Eip4844
    {
        Some(transaction.blob_hashes().to_vec())
    } else {
        None
    };

    L1CallRequest {
        from: Some(*from),
        to: transaction.kind().to().copied(),
        max_fee_per_gas: transaction.max_fee_per_gas().copied(),
        max_priority_fee_per_gas: transaction.max_priority_fee_per_gas().cloned(),
        gas: Some(transaction.gas_limit()),
        value: Some(*transaction.value()),
        data: Some(transaction.data().clone()),
        access_list: transaction
            .access_list()
            .map(<[edr_eip2930::AccessListItem]>::to_vec),
        blobs,
        blob_hashes,
        ..L1CallRequest::default()
    }
}

fn fake_transaction_request() -> TransactionRequest {
    let transaction = fake_pooled_transaction();
    let blobs = transaction.blobs().map(<[edr_eth::Blob]>::to_vec);

    let transaction = transaction.into_payload();
    let from = *transaction.caller();

    let blob_hashes = if transaction.transaction_type() == edr_chain_l1::L1TransactionType::Eip4844
    {
        Some(transaction.blob_hashes().to_vec())
    } else {
        None
    };

    TransactionRequest {
        from,
        to: transaction.kind().to().copied(),
        max_fee_per_gas: transaction.max_fee_per_gas().copied(),
        max_priority_fee_per_gas: transaction.max_priority_fee_per_gas().cloned(),
        gas: Some(transaction.gas_limit()),
        value: Some(*transaction.value()),
        data: Some(transaction.data().clone()),
        nonce: Some(transaction.nonce()),
        chain_id: transaction.chain_id(),
        access_list: transaction
            .access_list()
            .map(<[edr_eip2930::AccessListItem]>::to_vec),
        transaction_type: Some(transaction.transaction_type().into()),
        blobs,
        blob_hashes,
        ..TransactionRequest::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn call_unsupported() -> anyhow::Result<()> {
    let request = fake_call_request();

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.hardfork = edr_chain_l1::Hardfork::SHANGHAI;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let error = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            request, None, None,
        )))
        .expect_err("Must return an error");

    assert!(matches!(
        error,
        ProviderError::Eip4844CallRequestUnsupported
    ));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas_unsupported() -> anyhow::Result<()> {
    let request = fake_call_request();

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.hardfork = edr_chain_l1::Hardfork::SHANGHAI;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let error = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
            request, None,
        )))
        .expect_err("Must return an error");

    assert!(matches!(
        error,
        ProviderError::Eip4844CallRequestUnsupported
    ));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_transaction_unsupported() -> anyhow::Result<()> {
    let transaction = fake_transaction_request();

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = transaction.chain_id.expect("Blob transaction has chain ID");

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let error = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(transaction),
        ))
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

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = expected.chain_id().expect("Blob transaction has chain ID");

    config.genesis_state.insert(
        secret_key_to_address(SECRET_KEYS[0])?,
        AccountOverride {
            balance: Some(one_ether()),
            ..AccountOverride::default()
        },
    );

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let result = provider.handle_request(ProviderRequest::with_single(
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

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = expected.chain_id().expect("Blob transaction has chain ID");

    config.genesis_state.insert(
        secret_key_to_address(SECRET_KEYS[0])?,
        AccountOverride {
            balance: Some(one_ether()),
            ..AccountOverride::default()
        },
    );

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendRawTransaction(raw_eip4844_transaction),
    ))?;

    let transaction_hash: B256 = serde_json::from_value(result.result)?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionByHash(transaction_hash),
    ))?;

    let transaction: L1RpcTransactionWithSignature = serde_json::from_value(result.result)?;
    let transaction = edr_chain_l1::L1SignedTransaction::try_from(transaction)?;

    assert_eq!(transaction, expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn block_header() -> anyhow::Result<()> {
    let raw_eip4844_transaction = fake_raw_transaction();

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = fake_transaction()
        .chain_id()
        .expect("Blob transaction has chain ID");
    config.hardfork = edr_chain_l1::Hardfork::CANCUN;

    config.genesis_state.insert(
        secret_key_to_address(SECRET_KEYS[0])?,
        AccountOverride {
            balance: Some(one_ether()),
            ..AccountOverride::default()
        },
    );

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

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendRawTransaction(raw_eip4844_transaction),
    ))?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let first_block: L1RpcBlock<B256> = serde_json::from_value(result.result)?;
    assert_eq!(first_block.blob_gas_used, Some(DATA_GAS_PER_BLOB));

    assert_eq!(
        first_block.excess_blob_gas,
        Some(excess_blobs * DATA_GAS_PER_BLOB)
    );

    // The first block does not affect the number of excess blobs, as it has less
    // than the target number of blobs (3)

    let excess_blob_transaction = BlobTransactionBuilder::default()
        .duplicate_blobs(4)
        .nonce(1)
        .build_raw();

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendRawTransaction(excess_blob_transaction),
    ))?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let second_block: L1RpcBlock<B256> = serde_json::from_value(result.result)?;
    assert_eq!(second_block.blob_gas_used, Some(4 * DATA_GAS_PER_BLOB));

    assert_eq!(
        second_block.excess_blob_gas,
        Some(excess_blobs * DATA_GAS_PER_BLOB)
    );

    // The second block increases the excess by 1 blob (4 - 3)
    excess_blobs += 1;

    let excess_blob_transaction = BlobTransactionBuilder::default()
        .duplicate_blobs(5)
        .nonce(2)
        .build_raw();

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendRawTransaction(excess_blob_transaction),
    ))?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let third_block: L1RpcBlock<B256> = serde_json::from_value(result.result)?;
    assert_eq!(third_block.blob_gas_used, Some(5 * DATA_GAS_PER_BLOB));

    assert_eq!(
        third_block.excess_blob_gas,
        Some(excess_blobs * DATA_GAS_PER_BLOB)
    );

    // The third block increases the excess by 2 blob (5 - 3)
    excess_blobs += 2;

    // Mine an empty block to validate the previous block's excess
    provider.handle_request(ProviderRequest::with_single(MethodInvocation::Mine(
        None, None,
    )))?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let fourth_block: L1RpcBlock<B256> = serde_json::from_value(result.result)?;
    assert_eq!(fourth_block.blob_gas_used, Some(0u64));

    assert_eq!(
        fourth_block.excess_blob_gas,
        Some(excess_blobs * DATA_GAS_PER_BLOB)
    );

    // The fourth block decreases the excess by 3 blob (0 - 3), but should not go
    // below 0 - the minimum
    excess_blobs = excess_blobs.saturating_sub(3);

    // Mine an empty block to validate the previous block's excess
    provider.handle_request(ProviderRequest::with_single(MethodInvocation::Mine(
        None, None,
    )))?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let fifth_block: L1RpcBlock<B256> = serde_json::from_value(result.result)?;
    assert_eq!(fifth_block.blob_gas_used, Some(0u64));

    assert_eq!(
        fifth_block.excess_blob_gas,
        Some(excess_blobs * DATA_GAS_PER_BLOB)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_hash_opcode() -> anyhow::Result<()> {
    fn assert_blob_hash_opcodes(
        provider: &Provider<L1ChainSpec>,
        contract_address: &Address,
        num_blobs: usize,
        nonce: u64,
    ) -> anyhow::Result<()> {
        let builder = BlobTransactionBuilder::default()
            .duplicate_blobs(num_blobs)
            .input(Bytes::from_str("0x2069b0c7")?)
            .nonce(nonce)
            .to(*contract_address);

        let blob_hashes = builder.blob_hashes();
        let call_transaction = builder.build_raw();

        provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::SendRawTransaction(call_transaction),
        ))?;

        for (idx, blob_hash) in blob_hashes.into_iter().enumerate() {
            let index = U256::from(idx);

            let result = provider.handle_request(ProviderRequest::with_single(
                MethodInvocation::GetStorageAt(*contract_address, index, None),
            ))?;

            let storage_value: B256 = serde_json::from_value(result.result)?;
            assert_eq!(storage_value, blob_hash);
        }

        for idx in num_blobs..6 {
            let index = U256::from(idx);

            let result = provider.handle_request(ProviderRequest::with_single(
                MethodInvocation::GetStorageAt(*contract_address, index, None),
            ))?;

            let storage_value: B256 = serde_json::from_value(result.result)?;
            assert_eq!(storage_value, B256::ZERO);
        }

        Ok(())
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ContractFixture {
        _source: String,
        bytecode: Bytes,
    }

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = fake_transaction()
        .chain_id()
        .expect("Blob transaction has chain ID");

    let caller = secret_key_to_address(SECRET_KEYS[0])?;
    config.genesis_state.insert(
        caller,
        AccountOverride {
            balance: Some(one_ether()),
            ..AccountOverride::default()
        },
    );

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let fixture: ContractFixture =
        serde_json::from_str(include_str!("../fixtures/blob_hash_opcode_contract.json"))?;

    let contract_address = deploy_contract(&provider, caller, fixture.bytecode)?;

    let mut nonce = 1;
    for num_blobs in 1..=6 {
        assert_blob_hash_opcodes(&provider, &contract_address, num_blobs, nonce)?;
        nonce += 1;
    }

    Ok(())
}
