#![cfg(feature = "test-remote")]

use std::sync::Arc;

use anyhow::anyhow;
use edr_block_api::Block as _;
use edr_block_header::BlockConfig;
use edr_chain_spec_block::RemoteBlockForChainSpec;
use edr_chain_spec_provider::ProviderChainSpec;
use edr_defaults::CACHE_DIR;
use edr_eth::PreEip1898BlockSpec;
use edr_op::{
    hardfork,
    transaction::{signed::OpSignedTransaction, OpTransactionType},
    OpChainSpec,
};
use edr_primitives::{b256, B256};
use edr_provider::spec::ForkedBlockchainForChainSpec;
use edr_rpc_eth::client::EthRpcClientForChainSpec;
use edr_state_api::irregular::IrregularState;
use edr_test_utils::env::get_alchemy_url;
use edr_transaction::TransactionType as _;
use edr_utils::random::RandomHashGenerator;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn block_with_transactions() -> anyhow::Result<()> {
    const BLOCK_NUMBER_WITH_TRANSACTIONS: u64 = 117_156_000;

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = EthRpcClientForChainSpec::<OpChainSpec>::new(&url, CACHE_DIR.into(), None)?;

    let block = rpc_client
        .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::Number(
            BLOCK_NUMBER_WITH_TRANSACTIONS,
        ))
        .await?;

    let _block = RemoteBlockForChainSpec::<OpChainSpec>::new(
        block,
        Arc::new(rpc_client),
        runtime::Handle::current(),
    )?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn block_with_deposit_transaction() -> anyhow::Result<()> {
    const BLOCK_NUMBER_WITH_DEPOSIT: u64 = 121_874_088;
    const CHAIN_ID: u64 = 10;

    let runtime = tokio::runtime::Handle::current();

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = EthRpcClientForChainSpec::<OpChainSpec>::new(&url, CACHE_DIR.into(), None)?;
    let rpc_client = Arc::new(rpc_client);

    let replay_block = {
        let block = rpc_client
            .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::Number(
                BLOCK_NUMBER_WITH_DEPOSIT,
            ))
            .await?;

        RemoteBlockForChainSpec::<OpChainSpec>::new(block, rpc_client.clone(), runtime.clone())?
    };

    let mut irregular_state = IrregularState::default();
    let state_root_generator = Arc::new(parking_lot::Mutex::new(RandomHashGenerator::with_seed(
        edr_defaults::STATE_ROOT_HASH_SEED,
    )));

    let chain_config =
        hardfork::op_chain_config(CHAIN_ID).ok_or(anyhow!("Unsupported chain id"))?;

    let hardfork = chain_config
        .hardfork_activations
        .hardfork_at_block(
            BLOCK_NUMBER_WITH_DEPOSIT,
            replay_block.block_header().timestamp,
        )
        .ok_or(anyhow!("Unsupported block"))?;

    let _blockchain = ForkedBlockchainForChainSpec::<OpChainSpec>::new(
        BlockConfig {
            base_fee_params: &chain_config.base_fee_params,
            hardfork,
            min_ethash_difficulty: OpChainSpec::MIN_ETHASH_DIFFICULTY,
        },
        runtime.clone(),
        rpc_client.clone(),
        &mut irregular_state,
        state_root_generator,
        OpChainSpec::chain_configs(),
        Some(BLOCK_NUMBER_WITH_DEPOSIT - 1),
        None,
    )
    .await?;

    Ok(())
}

// TODO: https://github.com/NomicFoundation/edr/issues/1112
#[tokio::test(flavor = "multi_thread")]
async fn deposit_transaction_and_receipt_regolith() -> anyhow::Result<()> {
    const TRANSACTION_HASH: B256 =
        b256!("dd8e089476419b44cc37d72e631c44c57b38ac5a25fe5dea7b38688b83022fa1");

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = EthRpcClientForChainSpec::<OpChainSpec>::new(&url, CACHE_DIR.into(), None)?;

    let transaction = rpc_client
        .get_transaction_by_hash(TRANSACTION_HASH)
        .await?
        .expect("Transaction must exist");

    let transaction = OpSignedTransaction::try_from(transaction)?;
    assert_eq!(transaction.transaction_type(), OpTransactionType::Deposit);

    let receipt = rpc_client
        .get_transaction_receipt(TRANSACTION_HASH)
        .await?
        .expect("Receipt must exist");

    assert_eq!(
        receipt.transaction_type,
        Some(OpTransactionType::Deposit.into())
    );
    assert!(receipt.deposit_receipt_version.is_none());

    let l1_block_info = &receipt.l1_block_info;
    assert_eq!(l1_block_info.l1_gas_price, Some(0x758c0b711));
    assert_eq!(l1_block_info.l1_gas_used, Some(0xcf0));
    assert_eq!(l1_block_info.l1_fee, Some(0x0));
    assert_eq!(l1_block_info.l1_fee_scalar, Some(0.684));
    assert_eq!(l1_block_info.l1_base_fee_scalar, Some(0xa6fe0));
    assert!(l1_block_info.l1_blob_base_fee.is_none());
    assert!(l1_block_info.l1_blob_base_fee_scalar.is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_transaction_and_receipt_canyon() -> anyhow::Result<()> {
    const TRANSACTION_HASH: B256 =
        b256!("64c32c8d474e8befdea12e25338ad86d53950b1156c413f409e785112cfed4d3");

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = EthRpcClientForChainSpec::<OpChainSpec>::new(&url, CACHE_DIR.into(), None)?;

    let transaction = rpc_client
        .get_transaction_by_hash(TRANSACTION_HASH)
        .await?
        .expect("Transaction must exist");

    let transaction = OpSignedTransaction::try_from(transaction)?;
    assert_eq!(transaction.transaction_type(), OpTransactionType::Deposit);

    let receipt = rpc_client
        .get_transaction_receipt(TRANSACTION_HASH)
        .await?
        .expect("Receipt must exist");

    assert_eq!(
        receipt.transaction_type,
        Some(OpTransactionType::Deposit.into())
    );
    assert_eq!(receipt.deposit_receipt_version, Some(1));

    let l1_block_info = &receipt.l1_block_info;
    assert_eq!(l1_block_info.l1_gas_price, Some(0x221d9108d));
    assert_eq!(l1_block_info.l1_gas_used, Some(0xcf0));
    assert_eq!(l1_block_info.l1_fee, Some(0x0));
    assert_eq!(l1_block_info.l1_fee_scalar, Some(0.684));
    assert_eq!(l1_block_info.l1_base_fee_scalar, Some(0xa6fe0));
    assert!(l1_block_info.l1_blob_base_fee.is_none());
    assert!(l1_block_info.l1_blob_base_fee_scalar.is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_transaction_and_receipt_ecotone() -> anyhow::Result<()> {
    const TRANSACTION_HASH: B256 =
        b256!("cca2f31992022e3a833959c505de021285a7c5339c8d1b8ad75100074e1c6aea");

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = EthRpcClientForChainSpec::<OpChainSpec>::new(&url, CACHE_DIR.into(), None)?;

    let transaction = rpc_client
        .get_transaction_by_hash(TRANSACTION_HASH)
        .await?
        .expect("Transaction must exist");

    let transaction = OpSignedTransaction::try_from(transaction)?;
    assert_eq!(transaction.transaction_type(), OpTransactionType::Deposit);

    let receipt = rpc_client
        .get_transaction_receipt(TRANSACTION_HASH)
        .await?
        .expect("Receipt must exist");

    assert_eq!(
        receipt.transaction_type,
        Some(OpTransactionType::Deposit.into())
    );
    assert_eq!(receipt.deposit_receipt_version, Some(1));

    let l1_block_info = &receipt.l1_block_info;
    assert_eq!(l1_block_info.l1_gas_price, Some(0x17a2aaed0));
    assert_eq!(l1_block_info.l1_gas_used, Some(0xaac));
    assert_eq!(l1_block_info.l1_fee, Some(0x0));
    assert_eq!(l1_block_info.l1_fee_scalar, None);
    assert_eq!(l1_block_info.l1_base_fee_scalar, Some(0x558));
    assert_eq!(l1_block_info.l1_blob_base_fee, Some(0x1));
    assert_eq!(l1_block_info.l1_blob_base_fee_scalar, Some(0xc5fc5));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn receipt_with_l1_block_info() -> anyhow::Result<()> {
    const TRANSACTION_HASH: B256 =
        b256!("f0b04a1c6f61b2818ac2c62ed0c3fc22cd7ebd2f51161759714f75dd27fa7caa");

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = EthRpcClientForChainSpec::<OpChainSpec>::new(&url, CACHE_DIR.into(), None)?;

    let receipt = rpc_client
        .get_transaction_receipt(TRANSACTION_HASH)
        .await?
        .expect("Receipt must exist");

    assert_eq!(receipt.l1_block_info.l1_gas_price, Some(0x5f3a77dd6));
    assert_eq!(receipt.l1_block_info.l1_gas_used, Some(0x640));
    assert_eq!(receipt.l1_block_info.l1_fee, Some(0x1c3441e5e02));
    assert_eq!(receipt.l1_block_info.l1_fee_scalar, None);
    assert_eq!(receipt.l1_block_info.l1_base_fee_scalar, Some(0x146b));
    assert_eq!(receipt.l1_block_info.l1_blob_base_fee, Some(0x3f5694c1f));
    assert_eq!(receipt.l1_block_info.l1_blob_base_fee_scalar, Some(0xf79c5));

    Ok(())
}
