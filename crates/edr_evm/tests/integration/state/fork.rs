use std::{
    ops::{Deref, DerefMut},
    str::FromStr as _,
    sync::Arc,
};

use edr_chain_l1::L1ChainSpec;
use edr_eth::{Address, PreEip1898BlockSpec, B256, U256};
use edr_evm::{
    state::{ForkState, State as _, StateDebug as _},
    RandomHashGenerator,
};
use edr_rpc_eth::client::EthRpcClient;
use edr_test_utils::env::get_alchemy_url;
use parking_lot::Mutex;
use tokio::runtime;

const FORK_BLOCK: u64 = 16220843;

struct ForkStateFixture {
    fork_state: ForkState<L1ChainSpec>,
    // We need to keep it around as long as the fork state is alive
    _tempdir: tempfile::TempDir,
}

impl ForkStateFixture {
    /// Constructs a fork state for testing purposes.
    ///
    /// # Panics
    ///
    /// If the function is called without a `tokio::Runtime` existing.
    async fn new(fork_block_number: u64) -> Self {
        let hash_generator = Arc::new(Mutex::new(RandomHashGenerator::with_seed("seed")));

        let tempdir = tempfile::tempdir().expect("can create tempdir");

        let runtime = runtime::Handle::current();
        let rpc_client = EthRpcClient::<L1ChainSpec>::new(
            &get_alchemy_url(),
            tempdir.path().to_path_buf(),
            None,
        )
        .expect("url ok");

        let block = rpc_client
            .get_block_by_number(PreEip1898BlockSpec::Number(fork_block_number))
            .await
            .expect("failed to retrieve block by number")
            .expect("block should exist");

        let fork_state = ForkState::new(
            runtime,
            Arc::new(rpc_client),
            hash_generator,
            fork_block_number,
            block.state_root,
        );

        Self {
            fork_state,
            _tempdir: tempdir,
        }
    }
}

impl Deref for ForkStateFixture {
    type Target = ForkState<L1ChainSpec>;

    fn deref(&self) -> &Self::Target {
        &self.fork_state
    }
}

impl DerefMut for ForkStateFixture {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fork_state
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn basic_success() {
    let fork_state = ForkStateFixture::new(FORK_BLOCK).await;

    let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
        .expect("failed to parse address");

    let account_info = fork_state
        .basic(dai_address)
        .expect("should have succeeded");
    assert!(account_info.is_some());

    let account_info = account_info.unwrap();
    assert_eq!(account_info.balance, U256::from(0));
    assert_eq!(account_info.nonce, 1);
    assert_eq!(
        account_info.code_hash,
        B256::from_str("0x4e36f96ee1667a663dfaac57c4d185a0e369a3a217e0079d49620f34f85d1ac7")
            .expect("failed to parse")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn remove_remote_account_success() {
    let mut fork_state = ForkStateFixture::new(FORK_BLOCK).await;

    let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
        .expect("failed to parse address");

    fork_state.remove_account(dai_address).unwrap();

    assert_eq!(fork_state.basic(dai_address).unwrap(), None);
}
