use edr_eth::chain_spec::L1ChainSpec;
use edr_evm::test_utils::run_full_block;
use edr_rpc_eth::client::EthRpcClient;

pub async fn replay(url: String, block_number: Option<u64>, chain_id: u64) -> anyhow::Result<()> {
    let rpc_client = EthRpcClient::<L1ChainSpec>::new(&url, edr_defaults::CACHE_DIR.into(), None)?;

    let block_number = if let Some(block_number) = block_number {
        block_number
    } else {
        rpc_client
            .block_number()
            .await
            .map(|block_number| block_number - 20)?
    };

    println!("Testing block {block_number}");
    run_full_block::<L1ChainSpec>(url, block_number, chain_id).await
}
