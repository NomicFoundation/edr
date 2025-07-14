#![cfg(feature = "test-remote")]

use std::{str::FromStr, sync::Arc};

use edr_evm::state::{CachedRemoteState, StateMut as _};
use edr_rpc_eth::client::EthRpcClient;
use edr_test_utils::env::get_alchemy_url;
use tokio::runtime;

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn no_cache_for_unsafe_block_number() {
    let tempdir = tempfile::tempdir().expect("can create tempdir");

    let rpc_client =
        EthRpcClient::<L1ChainSpec>::new(&get_alchemy_url(), tempdir.path().to_path_buf(), None)
            .expect("url ok");

    let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
        .expect("failed to parse address");

    // Latest block number is always unsafe
    let block_number = rpc_client.block_number().await.unwrap();

    let runtime = runtime::Handle::current();

    let remote = RemoteState::new(runtime, Arc::new(rpc_client), block_number);
    let mut cached = CachedRemoteState::new(remote);

    let account_info = cached
        .basic_mut(dai_address)
        .expect("should succeed")
        .unwrap();

    cached
        .storage_mut(dai_address, U256::from(0))
        .expect("should succeed");

    for entry in cached.cache().values() {
        assert!(entry.is_empty());
    }

    cached
        .code_by_hash_mut(account_info.code_hash)
        .expect("should succeed");
}
