#![cfg(feature = "test-remote")]

use std::sync::Arc;

use edr_chain_l1::L1ChainSpec;
use edr_evm::{blockchain::remote::RemoteBlockchain, RemoteBlock};
use edr_rpc_eth::client::EthRpcClient;
use edr_test_utils::env::get_alchemy_url;
use tokio::runtime;

#[tokio::test]
async fn no_cache_for_unsafe_block_number() {
    let tempdir = tempfile::tempdir().expect("can create tempdir");

    let rpc_client =
        EthRpcClient::<L1ChainSpec>::new(&get_alchemy_url(), tempdir.path().to_path_buf(), None)
            .expect("url ok");

    // Latest block number is always unsafe to cache
    let block_number = rpc_client.block_number().await.unwrap();

    let remote = RemoteBlockchain::<RemoteBlock<L1ChainSpec>, L1ChainSpec, false>::new(
        Arc::new(rpc_client),
        runtime::Handle::current(),
    );

    let _ = remote.block_by_number(block_number).await.unwrap();
    assert!(remote.cache().await.block_by_number(block_number).is_none());
}
