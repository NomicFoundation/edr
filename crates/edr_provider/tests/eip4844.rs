#![cfg(feature = "test-utils")]

use std::{convert::Infallible, str::FromStr};

use edr_defaults::SECRET_KEYS;
use edr_eth::{
    receipt::BlockReceipt,
    remote::{self, PreEip1898BlockSpec},
    rlp::{self, Decodable},
    signature::{secret_key_from_str, secret_key_to_address},
    transaction::{
        pooled::{
            eip4844::{Blob, Bytes48, BYTES_PER_BLOB},
            Eip4844PooledTransaction, PooledTransaction,
        },
        Eip4844TransactionRequest, EthTransactionRequest, SignedTransaction, Transaction,
    },
    AccountInfo, Address, Bytes, B256, U256,
};
use edr_evm::{EnvKzgSettings, ExecutableTransaction, KECCAK_EMPTY};
use edr_provider::{
    test_utils::{create_test_config, one_ether},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderError, ProviderRequest,
};
use tokio::runtime;

/// Helper struct to modify the pooled transaction from the value in
/// `fixtures/eip4844.txt`. It reuses the secret key from `SECRET_KEYS[0]`.
struct BlobTransactionBuilder {
    request: Eip4844TransactionRequest,
    blobs: Vec<Blob>,
    commitments: Vec<Bytes48>,
    proofs: Vec<Bytes48>,
}

impl BlobTransactionBuilder {
    pub fn blob_hashes(&self) -> Vec<B256> {
        self.request.blob_hashes.clone()
    }

    pub fn build(self) -> PooledTransaction {
        let secret_key = secret_key_from_str(SECRET_KEYS[0]).expect("Invalid secret key");
        let signed_transaction = self
            .request
            .sign(&secret_key)
            .expect("Failed to sign transaction");

        let settings = EnvKzgSettings::Default;
        let pooled_transaction = Eip4844PooledTransaction::new(
            signed_transaction,
            self.blobs,
            self.commitments,
            self.proofs,
            settings.get(),
        )
        .expect("Invalid blob transaction");

        PooledTransaction::Eip4844(pooled_transaction)
    }

    pub fn build_raw(self) -> Bytes {
        rlp::encode(self.build()).into()
    }

    /// Duplicates the blobs, commitments, and proofs such that they exist
    /// `count` times.
    pub fn duplicate_blobs(mut self, count: usize) -> Self {
        self.request.blob_hashes = self
            .request
            .blob_hashes
            .into_iter()
            .cycle()
            .take(count)
            .collect();

        self.blobs = self.blobs.into_iter().cycle().take(count).collect();
        self.commitments = self.commitments.into_iter().cycle().take(count).collect();
        self.proofs = self.proofs.into_iter().cycle().take(count).collect();

        self
    }

    pub fn input(mut self, input: Bytes) -> Self {
        self.request.input = input;
        self
    }

    pub fn nonce(mut self, nonce: u64) -> Self {
        self.request.nonce = nonce;
        self
    }

    pub fn to(mut self, to: Address) -> Self {
        self.request.to = to;
        self
    }
}

impl Default for BlobTransactionBuilder {
    fn default() -> Self {
        let PooledTransaction::Eip4844(pooled_transaction) = fake_pooled_transaction() else {
            unreachable!("Must be an EIP-4844 transaction")
        };

        let (transaction, blobs, commitments, proofs) = pooled_transaction.into_inner();

        let request = Eip4844TransactionRequest::from(&transaction);

        Self {
            request,
            blobs,
            commitments,
            proofs,
        }
    }
}

// const CONTRACT_ADDRESS: Address = todo!();

/// Must match the value in `fixtures/eip4844.txt`. The transaction was signed
/// by private key `SECRETS[0]`
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

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        CurrentTime,
    )?;

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
        secret_key_to_address(SECRET_KEYS[0])?,
        AccountInfo {
            balance: one_ether(),
            nonce: 0,
            code: None,
            code_hash: KECCAK_EMPTY,
        },
    );

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        CurrentTime,
    )?;

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
        secret_key_to_address(SECRET_KEYS[0])?,
        AccountInfo {
            balance: one_ether(),
            nonce: 0,
            code: None,
            code_hash: KECCAK_EMPTY,
        },
    );

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        CurrentTime,
    )?;

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
        secret_key_to_address(SECRET_KEYS[0])?,
        AccountInfo {
            balance: one_ether(),
            nonce: 0,
            code: None,
            code_hash: KECCAK_EMPTY,
        },
    );

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
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

    let first_block: remote::eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(first_block.blob_gas_used, Some(BYTES_PER_BLOB as u64));

    assert_eq!(
        first_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    // The first block does not affect the number of excess blobs, as it has less
    // than the target number of blobs (3)

    let excess_blob_transaction = BlobTransactionBuilder::default()
        .duplicate_blobs(4)
        .nonce(1)
        .build_raw();

    provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendRawTransaction(excess_blob_transaction),
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

    let excess_blob_transaction = BlobTransactionBuilder::default()
        .duplicate_blobs(5)
        .nonce(2)
        .build_raw();

    provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendRawTransaction(excess_blob_transaction),
    ))?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let third_block: remote::eth::Block<B256> = serde_json::from_value(result.result)?;
    assert_eq!(third_block.blob_gas_used, Some(5 * BYTES_PER_BLOB as u64));

    assert_eq!(
        third_block.excess_blob_gas,
        Some(excess_blobs * BYTES_PER_BLOB as u64)
    );

    // The third block increases the excess by 2 blob (5 - 3)
    excess_blobs += 2;

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

#[tokio::test(flavor = "multi_thread")]
async fn blob_hash_opcode() -> anyhow::Result<()> {
    fn assert_blob_hash_opcodes(
        provider: &Provider<Infallible>,
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

        provider.handle_request(ProviderRequest::Single(
            MethodInvocation::SendRawTransaction(call_transaction),
        ))?;

        for (idx, blob_hash) in blob_hashes.into_iter().enumerate() {
            let index = U256::from(idx);

            let result = provider.handle_request(ProviderRequest::Single(
                MethodInvocation::GetStorageAt(*contract_address, index, None),
            ))?;

            let storage_value: B256 = serde_json::from_value(result.result)?;
            assert_eq!(storage_value, blob_hash);
        }

        for idx in num_blobs..6 {
            let index = U256::from(idx);

            let result = provider.handle_request(ProviderRequest::Single(
                MethodInvocation::GetStorageAt(*contract_address, index, None),
            ))?;

            let storage_value: B256 = serde_json::from_value(result.result)?;
            assert_eq!(storage_value, B256::ZERO);
        }

        Ok(())
    }

    const CONTRACT_CODE: &str = include_str!("fixtures/blob_hash_opcode_contract.txt");

    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.chain_id = fake_transaction()
        .chain_id()
        .expect("Blob transaction has chain ID");

    let caller = secret_key_to_address(SECRET_KEYS[0])?;
    config.genesis_accounts.insert(
        caller,
        AccountInfo {
            balance: one_ether(),
            nonce: 0,
            code: None,
            code_hash: KECCAK_EMPTY,
        },
    );

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        CurrentTime,
    )?;

    let deploy_transaction = EthTransactionRequest {
        from: caller,
        data: Some(Bytes::from_str(CONTRACT_CODE)?),
        ..EthTransactionRequest::default()
    };

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendTransaction(deploy_transaction),
    ))?;

    let transaction_hash: B256 = serde_json::from_value(result.result)?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetTransactionReceipt(transaction_hash),
    ))?;

    let receipt: Option<BlockReceipt> = serde_json::from_value(result.result)?;
    let receipt = receipt.expect("Transaction receipt must exist");
    let contract_address = receipt.contract_address.expect("Call must create contract");

    let mut nonce = 1;
    for num_blobs in 1..=6 {
        assert_blob_hash_opcodes(&provider, &contract_address, num_blobs, nonce)?;
        nonce += 1;
    }

    Ok(())
}