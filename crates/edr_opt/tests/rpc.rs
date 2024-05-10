#![cfg(feature = "test-remote")]

use edr_defaults::CACHE_DIR;
use edr_eth::remote::{self, RpcClient};
use edr_test_utils::env::get_alchemy_url;

#[tokio::test(flavor = "multi_thread")]
async fn block() -> anyhow::Result<()> {
    const BLOCK_NUMBER_WITH_TRANSACTIONS: u64 = 117_156_000;

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = RpcClient::new(&url, CACHE_DIR.into(), None)?;

    let serialized = rpc_client.serialize_request(&remote::RequestMethod::GetBlockByNumber(
        BLOCK_NUMBER_WITH_TRANSACTIONS.into(),
        true,
    ))?;

    let block = rpc_client.send_request_body(&serialized).await?;

    println!("serialized: {block}");

    // .get_block_by_number(BLOCK_NUMBER_WITH_TRANSACTIONS.into())
    // .await?;

    Ok(())
}
