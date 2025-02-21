#![cfg(feature = "test-remote")]

use std::sync::Arc;

use edr_defaults::CACHE_DIR;
use edr_eth::{HashMap, SpecId};
use edr_evm::{
    blockchain::{Blockchain, ForkedBlockchain},
    chain_spec::L1ChainSpec,
    state::IrregularState,
    RandomHashGenerator,
};
use edr_rpc_eth::client::EthRpcClient;
use edr_test_utils::env::get_alchemy_url;
use parking_lot::Mutex;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn unknown_transaction_types() -> anyhow::Result<()> {
    const BLOCK_NUMBER_WITH_TRANSACTIONS: u64 = 117_156_000;

    let url = get_alchemy_url().replace("eth-", "opt-");
    // TODO: https://github.com/NomicFoundation/edr/issues/512
    // Change the spec to `OptimismChainSpec` once it's implemented
    let rpc_client = EthRpcClient::<L1ChainSpec>::new(&url, CACHE_DIR.into(), None)?;
    let mut irregular_state = IrregularState::default();
    let state_root_generator = Arc::new(Mutex::new(RandomHashGenerator::with_seed("test")));
    let hardfork_activation_overrides = HashMap::new();

    let blockchain = ForkedBlockchain::new(
        runtime::Handle::current(),
        None,
        SpecId::LATEST,
        Arc::new(rpc_client),
        None,
        &mut irregular_state,
        state_root_generator,
        &hardfork_activation_overrides,
    )
    .await?;

    let block_with_transactions = blockchain
        .block_by_number(BLOCK_NUMBER_WITH_TRANSACTIONS)?
        .expect("Block must exist");

    let _receipts = block_with_transactions.transaction_receipts()?;

    Ok(())
}
