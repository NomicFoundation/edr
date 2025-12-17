#![cfg(feature = "test-remote")]

use std::sync::Arc;

use edr_blockchain_api::GetBlockchainBlock as _;
use edr_chain_spec_provider::ProviderChainSpec;
use edr_defaults::CACHE_DIR;
use edr_generic::GenericChainSpec;
use edr_provider::spec::ForkedBlockchainForChainSpec;
use edr_rpc_eth::client::EthRpcClientForChainSpec;
use edr_state_api::irregular::IrregularState;
use edr_test_utils::env::get_alchemy_url;
use edr_utils::random::RandomHashGenerator;
use parking_lot::Mutex;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn unknown_transaction_types() -> anyhow::Result<()> {
    const BLOCK_NUMBER_WITH_TRANSACTIONS: u64 = 117_156_000;

    // Make sure that we do not error out when encountering unknown Ethereum
    // transaction types (e.g. found in OP), as we want to fallback to
    // legacy transactions for the for the generic (aka fallback) chain spec.
    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client =
        EthRpcClientForChainSpec::<GenericChainSpec>::new(&url, CACHE_DIR.into(), None)?;
    let mut irregular_state = IrregularState::default();
    let state_root_generator = Arc::new(Mutex::new(RandomHashGenerator::with_seed("test")));

    let blockchain = ForkedBlockchainForChainSpec::<GenericChainSpec>::new(
        edr_chain_l1::Hardfork::default(),
        runtime::Handle::current(),
        Arc::new(rpc_client),
        &mut irregular_state,
        state_root_generator,
        GenericChainSpec::chain_configs(),
        None,
        None,
    )
    .await?;

    let block_with_transactions = blockchain
        .block_by_number(BLOCK_NUMBER_WITH_TRANSACTIONS)?
        .expect("Block must exist");

    let _receipts = block_with_transactions.fetch_transaction_receipts()?;

    Ok(())
}
