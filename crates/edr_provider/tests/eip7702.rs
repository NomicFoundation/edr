#![cfg(feature = "test-utils")]

#[path = "eip7702/different_sender_and_authorizer.rs"]
mod different_sender_and_authorizer;
#[path = "eip7702/multiple_authorizers.rs"]
mod multiple_authorizers;
#[path = "eip7702/reset.rs"]
mod reset;
#[path = "eip7702/same_sender_and_authorizer.rs"]
mod same_sender_and_authorizer;
#[path = "eip7702/zeroed_chain_id.rs"]
mod zeroed_chain_id;

use std::{convert::Infallible, sync::Arc};

use edr_eth::{
    eips::eip7702,
    signature::{SecretKey, SignatureWithYParity},
    Address, Bytes,
};
use edr_provider::{
    time::CurrentTime, MethodInvocation, NoopLogger, Provider, ProviderConfig, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

const CHAIN_ID: u64 = 0x7a69;

fn assert_code_at(provider: &Provider<Infallible>, address: Address, expected: &Bytes) {
    let code: Bytes = {
        let response = provider
            .handle_request(ProviderRequest::Single(MethodInvocation::GetCode(
                address, None,
            )))
            .expect("eth_getCode should succeed");

        serde_json::from_value(response.result).expect("response should be Bytes")
    };

    assert_eq!(code, *expected);
}

fn new_provider(config: ProviderConfig) -> anyhow::Result<Provider<Infallible>> {
    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    Ok(provider)
}

fn sign_authorization(
    authorization: eip7702::Authorization,
    secret_key: &SecretKey,
) -> anyhow::Result<eip7702::SignedAuthorization> {
    let signature = SignatureWithYParity::with_message(authorization.signature_hash(), secret_key)?;

    Ok(authorization.into_signed(signature.into_inner()))
}

// #[tokio::test(flavor = "multi_thread")]
// async fn get_transaction() -> anyhow::Result<()> {
//     let raw_eip4844_transaction = fake_raw_transaction();

//     let expected = fake_transaction();

//     let logger = Box::new(NoopLogger);
//     let subscriber = Box::new(|_event| {});
//     let mut config = create_test_config();
//     config.chain_id = expected.chain_id().expect("Blob transaction has chain
// ID");

//     config.genesis_accounts.insert(
//         secret_key_to_address(SECRET_KEYS[0])?,
//         AccountInfo {
//             balance: one_ether(),
//             nonce: 0,
//             code: None,
//             code_hash: KECCAK_EMPTY,
//         },
//     );

//     let provider = Provider::new(
//         runtime::Handle::current(),
//         logger,
//         subscriber,
//         config,
//         Arc::<ContractDecoder>::default(),
//         CurrentTime,
//     )?;

//     let result = provider.handle_request(ProviderRequest::Single(
//         MethodInvocation::SendRawTransaction(raw_eip4844_transaction),
//     ))?;

//     let transaction_hash: B256 = serde_json::from_value(result.result)?;

//     let result = provider.handle_request(ProviderRequest::Single(
//         MethodInvocation::GetTransactionByHash(transaction_hash),
//     ))?;

//     let transaction: edr_rpc_eth::Transaction =
// serde_json::from_value(result.result)?;     let transaction =
// transaction::Signed::try_from(transaction)?;

//     assert_eq!(transaction, expected);

//     Ok(())
// }

// #[tokio::test(flavor = "multi_thread")]
// async fn block_header() -> anyhow::Result<()> {
//     let raw_eip4844_transaction = fake_raw_transaction();

//     let logger = Box::new(NoopLogger);
//     let subscriber = Box::new(|_event| {});
//     let mut config = create_test_config();
//     config.chain_id = fake_transaction()
//         .chain_id()
//         .expect("Blob transaction has chain ID");
//     config.hardfork = SpecId::CANCUN;

//     config.genesis_accounts.insert(
//         secret_key_to_address(SECRET_KEYS[0])?,
//         AccountInfo {
//             balance: one_ether(),
//             nonce: 0,
//             code: None,
//             code_hash: KECCAK_EMPTY,
//         },
//     );

//     let provider = Provider::new(
//         runtime::Handle::current(),
//         logger,
//         subscriber,
//         config,
//         Arc::<ContractDecoder>::default(),
//         CurrentTime,
//     )?;

//     // The genesis block has 0 excess blobs
//     let mut excess_blobs = 0u64;

//     provider.handle_request(ProviderRequest::Single(
//         MethodInvocation::SendRawTransaction(raw_eip4844_transaction),
//     ))?;

//     let result = provider.handle_request(ProviderRequest::Single(
//         MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(),
// false),     ))?;

//     let first_block: edr_rpc_eth::Block<B256> =
// serde_json::from_value(result.result)?;     assert_eq!(first_block.
// blob_gas_used, Some(BYTES_PER_BLOB as u64));

//     assert_eq!(
//         first_block.excess_blob_gas,
//         Some(excess_blobs * BYTES_PER_BLOB as u64)
//     );

//     // The first block does not affect the number of excess blobs, as it has
// less     // than the target number of blobs (3)

//     let excess_blob_transaction = BlobTransactionBuilder::default()
//         .duplicate_blobs(4)
//         .nonce(1)
//         .build_raw();

//     provider.handle_request(ProviderRequest::Single(
//         MethodInvocation::SendRawTransaction(excess_blob_transaction),
//     ))?;

//     let result = provider.handle_request(ProviderRequest::Single(
//         MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(),
// false),     ))?;

//     let second_block: edr_rpc_eth::Block<B256> =
// serde_json::from_value(result.result)?;     assert_eq!(second_block.
// blob_gas_used, Some(4 * BYTES_PER_BLOB as u64));

//     assert_eq!(
//         second_block.excess_blob_gas,
//         Some(excess_blobs * BYTES_PER_BLOB as u64)
//     );

//     // The second block increases the excess by 1 blob (4 - 3)
//     excess_blobs += 1;

//     let excess_blob_transaction = BlobTransactionBuilder::default()
//         .duplicate_blobs(5)
//         .nonce(2)
//         .build_raw();

//     provider.handle_request(ProviderRequest::Single(
//         MethodInvocation::SendRawTransaction(excess_blob_transaction),
//     ))?;

//     let result = provider.handle_request(ProviderRequest::Single(
//         MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(),
// false),     ))?;

//     let third_block: edr_rpc_eth::Block<B256> =
// serde_json::from_value(result.result)?;     assert_eq!(third_block.
// blob_gas_used, Some(5 * BYTES_PER_BLOB as u64));

//     assert_eq!(
//         third_block.excess_blob_gas,
//         Some(excess_blobs * BYTES_PER_BLOB as u64)
//     );

//     // The third block increases the excess by 2 blob (5 - 3)
//     excess_blobs += 2;

//     // Mine an empty block to validate the previous block's excess
//     provider.
// handle_request(ProviderRequest::Single(MethodInvocation::Mine(None, None)))?;

//     let result = provider.handle_request(ProviderRequest::Single(
//         MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(),
// false),     ))?;

//     let fourth_block: edr_rpc_eth::Block<B256> =
// serde_json::from_value(result.result)?;     assert_eq!(fourth_block.
// blob_gas_used, Some(0u64));

//     assert_eq!(
//         fourth_block.excess_blob_gas,
//         Some(excess_blobs * BYTES_PER_BLOB as u64)
//     );

//     // The fourth block decreases the excess by 3 blob (0 - 3), but should
// not go     // below 0 - the minimum
//     excess_blobs = excess_blobs.saturating_sub(3);

//     // Mine an empty block to validate the previous block's excess
//     provider.
// handle_request(ProviderRequest::Single(MethodInvocation::Mine(None, None)))?;

//     let result = provider.handle_request(ProviderRequest::Single(
//         MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(),
// false),     ))?;

//     let fifth_block: edr_rpc_eth::Block<B256> =
// serde_json::from_value(result.result)?;     assert_eq!(fifth_block.
// blob_gas_used, Some(0u64));

//     assert_eq!(
//         fifth_block.excess_blob_gas,
//         Some(excess_blobs * BYTES_PER_BLOB as u64)
//     );

//     Ok(())
// }

// #[tokio::test(flavor = "multi_thread")]
// async fn blob_hash_opcode() -> anyhow::Result<()> {
//     fn assert_blob_hash_opcodes(
//         provider: &Provider<Infallible>,
//         contract_address: &Address,
//         num_blobs: usize,
//         nonce: u64,
//     ) -> anyhow::Result<()> {
//         let builder = BlobTransactionBuilder::default()
//             .duplicate_blobs(num_blobs)
//             .input(Bytes::from_str("0x2069b0c7")?)
//             .nonce(nonce)
//             .to(*contract_address);

//         let blob_hashes = builder.blob_hashes();
//         let call_transaction = builder.build_raw();

//         provider.handle_request(ProviderRequest::Single(
//             MethodInvocation::SendRawTransaction(call_transaction),
//         ))?;

//         for (idx, blob_hash) in blob_hashes.into_iter().enumerate() {
//             let index = U256::from(idx);

//             let result = provider.handle_request(ProviderRequest::Single(
//                 MethodInvocation::GetStorageAt(*contract_address, index,
// None),             ))?;

//             let storage_value: B256 = serde_json::from_value(result.result)?;
//             assert_eq!(storage_value, blob_hash);
//         }

//         for idx in num_blobs..6 {
//             let index = U256::from(idx);

//             let result = provider.handle_request(ProviderRequest::Single(
//                 MethodInvocation::GetStorageAt(*contract_address, index,
// None),             ))?;

//             let storage_value: B256 = serde_json::from_value(result.result)?;
//             assert_eq!(storage_value, B256::ZERO);
//         }

//         Ok(())
//     }

//     #[derive(serde::Deserialize)]
//     #[serde(rename_all = "camelCase")]
//     struct ContractFixture {
//         _source: String,
//         bytecode: Bytes,
//     }

//     let logger = Box::new(NoopLogger);
//     let subscriber = Box::new(|_event| {});
//     let mut config = create_test_config();
//     config.chain_id = fake_transaction()
//         .chain_id()
//         .expect("Blob transaction has chain ID");

//     let caller = secret_key_to_address(SECRET_KEYS[0])?;
//     config.genesis_accounts.insert(
//         caller,
//         AccountInfo {
//             balance: one_ether(),
//             nonce: 0,
//             code: None,
//             code_hash: KECCAK_EMPTY,
//         },
//     );

//     let provider = Provider::new(
//         runtime::Handle::current(),
//         logger,
//         subscriber,
//         config,
//         Arc::<ContractDecoder>::default(),
//         CurrentTime,
//     )?;

//     let fixture: ContractFixture =
//         serde_json::from_str(include_str!("fixtures/
// blob_hash_opcode_contract.json"))?;

//     let contract_address = deploy_contract(&provider, caller,
// fixture.bytecode)?;

//     let mut nonce = 1;
//     for num_blobs in 1..=6 {
//         assert_blob_hash_opcodes(&provider, &contract_address, num_blobs,
// nonce)?;         nonce += 1;
//     }

//     Ok(())
// }
