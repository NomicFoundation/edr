#![cfg(feature = "test-utils")]

use std::{str::FromStr as _, sync::Arc};

use edr_defaults::SECRET_KEYS;
use edr_eth::{
    eips::eip7702,
    signature::{public_key_to_address, secret_key_from_str, SignatureWithYParity},
    transaction::EthTransactionRequest,
    Bytecode, Bytes, SpecId, U256,
};
use edr_evm::{address, bytes};
use edr_provider::{
    test_utils::{create_test_config, deploy_contract, one_ether},
    time::CurrentTime,
    AccountConfig, MethodInvocation, NoopLogger, Provider, ProviderError, ProviderRequest,
};
use edr_rpc_eth::CallRequest;
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

const CHAIN_ID: u64 = 0x7a69;

#[tokio::test(flavor = "multi_thread")]
async fn same_sender_and_signer() -> anyhow::Result<()> {
    static RAW_TRANSACTION: Bytes = bytes!("0x04f8cc827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85ef85c827a699412345678901234567890123456789012345678900101a0eb775e0a2b7a15ea4938921e1ab255c84270e25c2c384b2adc32c73cd70273d6a046b9bec1961318a644db6cd9c7fc4e8d7c6f40d9165fc8958f3aff2216ed6f7c01a0be47a039954e4dfb7f08927ef7f072e0ec7510290e3c4c1405f3bf0329d0be51a06f291c455321a863d4c8ebbd73d58e809328918bcb5555958247ca6ec27feec8");

    let secret_key = secret_key_from_str(SECRET_KEYS[0])?;
    let address = public_key_to_address(secret_key.public_key());

    let authorized_address = address!("0x1234567890123456789012345678901234567890");
    let authorization = eip7702::Authorization {
        chain_id: U256::from(CHAIN_ID),
        address: authorized_address,
        nonce: 0x1,
    };

    let signed_authorization = {
        let signature =
            SignatureWithYParity::with_message(authorization.signature_hash(), &secret_key)?;
        authorization.into_signed(signature.into_inner())
    };

    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});
    let mut config = create_test_config();
    config.accounts = vec![AccountConfig {
        secret_key,
        balance: one_ether(),
    }];
    // config.genesis_accounts.insert(
    //     address!("3d986a360ec7f4B6f81270aE1C24782f4F03D0C6"),
    //     AccountInfo {
    //         balance: one_ether(),
    //         ..AccountInfo::default()
    //     },
    // );
    config.chain_id = CHAIN_ID;
    config.hardfork = SpecId::PRAGUE;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    // let _response = provider
    //     .handle_request(ProviderRequest::Single(
    //         MethodInvocation::SendRawTransaction(RAW_TRANSACTION.clone()),
    //     ))
    //     .expect("eth_sendRawTransaction should succeed");

    let transaction_request = EthTransactionRequest {
        from: address,
        to: Some(address),
        authorization_list: Some(vec![signed_authorization.clone()]),
        ..EthTransactionRequest::default()
    };

    let _response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::SendTransaction(
            transaction_request,
        )))
        .expect("eth_sendTransaction should succeed");

    println!("{_response:?}");

    let code: Bytes = {
        let response = provider
            .handle_request(ProviderRequest::Single(MethodInvocation::GetCode(
                authorized_address,
                None,
            )))
            .expect("eth_getCode should succeed");

        serde_json::from_value(response.result)?
    };

    let expected = Bytes::from_str("0xef01001234567890123456789012345678901234567890")
        .expect("Valid bytecode");

    assert_eq!(code, expected);

    // let call_request = CallRequest {
    //     from: Some(address),
    //     to: Some(address),
    //     authorization_list: Some(vec![signed_authorization]),
    //     ..CallRequest::default()
    // };

    // let _response =
    // provider.handle_request(ProviderRequest::Single(MethodInvocation::Call(
    //     call_request,
    //     None,
    //     None,
    // )))?;

    Ok(())
}

// #[tokio::test(flavor = "multi_thread")]
// async fn send_raw_transaction() -> anyhow::Result<()> {
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
//     assert_eq!(transaction_hash, *expected.transaction_hash());

//     Ok(())
// }

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
