use std::{ops::Deref, str::FromStr};

use edr_chain_l1::L1ChainSpec;
use edr_primitives::B256;
use edr_rpc_client::RpcClientError;
use edr_rpc_eth::client::{EthRpcClient, EthRpcClientForChainSpec};
use reqwest::StatusCode;
use tempfile::TempDir;

struct TestRpcClient {
    client: EthRpcClientForChainSpec<L1ChainSpec>,

    // Need to keep the tempdir around to prevent it from being deleted
    // Only accessed when feature = "test-remote", hence the allow.
    #[allow(dead_code)]
    cache_dir: TempDir,
}

impl TestRpcClient {
    fn new(url: &str) -> Self {
        let tempdir = TempDir::new().unwrap();
        Self {
            client: EthRpcClient::new(url, tempdir.path().into(), None).expect("url ok"),
            cache_dir: tempdir,
        }
    }
}

impl Deref for TestRpcClient {
    type Target = EthRpcClientForChainSpec<L1ChainSpec>;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

#[tokio::test]
async fn send_request_body_400_status() {
    const STATUS_CODE: u16 = 400;

    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/")
        .with_status(STATUS_CODE.into())
        .with_header("content-type", "text/plain")
        .create_async()
        .await;

    let hash = B256::from_str("0xc008e9f9bb92057dd0035496fbf4fb54f66b4b18b370928e46d6603933022222")
        .expect("failed to parse hash from string");

    let error = TestRpcClient::new(&server.url())
        .get_transaction_by_hash(hash)
        .await
        .expect_err("should have failed to due to a HTTP status error");

    if let RpcClientError::HttpStatus(error) = error {
        assert_eq!(
            reqwest::Error::from(error).status(),
            Some(StatusCode::from_u16(STATUS_CODE).unwrap())
        );
    } else {
        unreachable!("Invalid error: {error}");
    }

    mock.assert_async().await;
}

#[cfg(feature = "test-remote")]
mod alchemy {
    use std::{fs::File, path::PathBuf};

    use edr_eth::{filter::OneOrMore, BlockSpec, PreEip1898BlockSpec};
    use edr_primitives::{Address, Bytes, U256};
    use edr_test_utils::env::get_alchemy_url;
    use walkdir::WalkDir;

    use super::*;

    // The maximum block number that Alchemy allows
    const MAX_BLOCK_NUMBER: u64 = u64::MAX >> 1;

    impl TestRpcClient {
        fn files_in_cache(&self) -> Vec<PathBuf> {
            let mut files = Vec::new();
            for entry in WalkDir::new(&self.cache_dir)
                .follow_links(true)
                .into_iter()
                .filter_map(Result::ok)
            {
                if entry.file_type().is_file() {
                    files.push(entry.path().to_owned());
                }
            }
            files
        }
    }

    #[tokio::test]
    async fn get_account_info_unknown_block() {
        let alchemy_url = get_alchemy_url();

        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");

        let error = TestRpcClient::new(&alchemy_url)
            .get_account_info(dai_address, Some(BlockSpec::Number(MAX_BLOCK_NUMBER)))
            .await
            .expect_err("should have failed");

        assert!(matches!(error, RpcClientError::JsonRpcError { .. }));
    }

    #[tokio::test]
    async fn get_account_infos() {
        let alchemy_url = get_alchemy_url();

        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");
        let hardhat_default_address =
            Address::from_str("0xbe862ad9abfe6f22bcb087716c7d89a26051f74c")
                .expect("failed to parse address");

        let account_infos = TestRpcClient::new(&alchemy_url)
            .get_account_infos(
                &[dai_address, hardhat_default_address],
                Some(BlockSpec::latest()),
            )
            .await
            .expect("should have succeeded");

        assert_eq!(account_infos.len(), 2);
    }

    #[tokio::test]
    async fn get_block_by_hash_some() {
        let alchemy_url = get_alchemy_url();

        let hash =
            B256::from_str("0x71d5e7c8ff9ea737034c16e333a75575a4a94d29482e0c2b88f0a6a8369c1812")
                .expect("failed to parse hash from string");

        let block = TestRpcClient::new(&alchemy_url)
            .get_block_by_hash(hash)
            .await
            .expect("should have succeeded");

        assert!(block.is_some());
        let block = block.unwrap();

        assert_eq!(block.hash, Some(hash));
        assert_eq!(block.transactions.len(), 192);
    }

    #[tokio::test]
    async fn get_block_by_hash_with_transaction_data_some() {
        let alchemy_url = get_alchemy_url();

        let hash =
            B256::from_str("0x71d5e7c8ff9ea737034c16e333a75575a4a94d29482e0c2b88f0a6a8369c1812")
                .expect("failed to parse hash from string");

        let block = TestRpcClient::new(&alchemy_url)
            .get_block_by_hash_with_transaction_data(hash)
            .await
            .expect("should have succeeded");

        assert!(block.is_some());
        let block = block.unwrap();

        assert_eq!(block.hash, Some(hash));
        assert_eq!(block.transactions.len(), 192);
    }

    #[tokio::test]
    async fn get_block_by_number_finalized_resolves() {
        let alchemy_url = get_alchemy_url();
        let client = TestRpcClient::new(&alchemy_url);

        assert_eq!(client.files_in_cache().len(), 0);

        client
            .get_block_by_number(PreEip1898BlockSpec::finalized())
            .await
            .expect("should have succeeded");

        // Finalized tag should be resolved and stored in cache.
        assert_eq!(client.files_in_cache().len(), 1);
    }

    #[tokio::test]
    async fn get_block_by_number_some() {
        let alchemy_url = get_alchemy_url();

        let block_number = 16222385;

        let block = TestRpcClient::new(&alchemy_url)
            .get_block_by_number(PreEip1898BlockSpec::Number(block_number))
            .await
            .expect("should have succeeded")
            .expect("Block must exist");

        assert_eq!(block.number, Some(block_number));
        assert_eq!(block.transactions.len(), 102);
    }

    #[tokio::test]
    async fn get_block_with_transaction_data_cached() {
        let alchemy_url = get_alchemy_url();
        let client = TestRpcClient::new(&alchemy_url);

        let block_spec = PreEip1898BlockSpec::Number(16220843);

        assert_eq!(client.files_in_cache().len(), 0);

        let block_from_remote = client
            .get_block_by_number_with_transaction_data(block_spec.clone())
            .await
            .expect("should have from remote");

        assert_eq!(client.files_in_cache().len(), 1);

        let block_from_cache = client
            .get_block_by_number_with_transaction_data(block_spec.clone())
            .await
            .expect("should have from remote");

        assert_eq!(block_from_remote, block_from_cache);
    }

    #[tokio::test]
    async fn get_earliest_block_with_transaction_data_resolves() {
        let alchemy_url = get_alchemy_url();
        let client = TestRpcClient::new(&alchemy_url);

        assert_eq!(client.files_in_cache().len(), 0);

        client
            .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::earliest())
            .await
            .expect("should have succeeded");

        // Earliest tag should be resolved to block number and it should be cached.
        assert_eq!(client.files_in_cache().len(), 1);
    }

    #[tokio::test]
    async fn get_latest_block() {
        let alchemy_url = get_alchemy_url();

        let _block = TestRpcClient::new(&alchemy_url)
            .get_block_by_number(PreEip1898BlockSpec::latest())
            .await
            .expect("should have succeeded");
    }

    #[tokio::test]
    async fn get_latest_block_with_transaction_data() {
        let alchemy_url = get_alchemy_url();

        let _block = TestRpcClient::new(&alchemy_url)
            .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::latest())
            .await
            .expect("should have succeeded");
    }

    #[tokio::test]
    async fn get_pending_block() {
        let alchemy_url = get_alchemy_url();

        let _block = TestRpcClient::new(&alchemy_url)
            .get_block_by_number(PreEip1898BlockSpec::pending())
            .await
            .expect("should have succeeded");
    }

    #[tokio::test]
    async fn get_pending_block_with_transaction_data() {
        let alchemy_url = get_alchemy_url();

        let _block = TestRpcClient::new(&alchemy_url)
            .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::pending())
            .await
            .expect("should have succeeded");
    }

    #[tokio::test]
    async fn get_logs_some() {
        let alchemy_url = get_alchemy_url();
        let logs = TestRpcClient::new(&alchemy_url)
            .get_logs_by_range(
                BlockSpec::Number(10496585),
                BlockSpec::Number(10496585),
                Some(OneOrMore::One(
                    Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
                        .expect("failed to parse data"),
                )),
                None,
            )
            .await
            .expect("failed to get logs");

        assert_eq!(logs.len(), 12);
        // TODO: assert more things about the log(s)
        // TODO: consider asserting something about the logs bloom
    }

    #[tokio::test]
    async fn get_logs_future_from_block() {
        let alchemy_url = get_alchemy_url();
        let result = TestRpcClient::new(&alchemy_url)
            .get_logs_by_range(
                BlockSpec::Number(MAX_BLOCK_NUMBER),
                BlockSpec::Number(MAX_BLOCK_NUMBER),
                Some(OneOrMore::One(
                    Address::from_str("0xffffffffffffffffffffffffffffffffffffffff")
                        .expect("failed to parse data"),
                )),
                None,
            )
            .await;

        // TODO: https://github.com/NomicFoundation/edr/issues/903
        // Alchemy enabled [EIP-4444](https://eips.ethereum.org/EIPS/eip-4444) for part of their clients. As a
        // result, it's possible that we get the updated result `[]` instead of the old
        // JSON-RPC error.
        match result {
            Ok(response) => {
                assert!(response.is_empty());
            }
            Err(error) => {
                assert!(matches!(error, RpcClientError::JsonRpcError { .. }));
            }
        }
    }

    #[tokio::test]
    async fn get_logs_future_to_block() {
        let alchemy_url = get_alchemy_url();
        let logs = TestRpcClient::new(&alchemy_url)
            .get_logs_by_range(
                BlockSpec::Number(10496585),
                BlockSpec::Number(MAX_BLOCK_NUMBER),
                Some(OneOrMore::One(
                    Address::from_str("0xffffffffffffffffffffffffffffffffffffffff")
                        .expect("failed to parse data"),
                )),
                None,
            )
            .await
            .expect("should have succeeded");

        assert_eq!(logs, []);
    }

    #[tokio::test]
    async fn get_transaction_by_hash_some() {
        let alchemy_url = get_alchemy_url();

        let hash =
            B256::from_str("0xc008e9f9bb92057dd0035496fbf4fb54f66b4b18b370928e46d6603933054d5a")
                .expect("failed to parse hash from string");

        let tx = TestRpcClient::new(&alchemy_url)
            .get_transaction_by_hash(hash)
            .await
            .expect("failed to get transaction by hash");

        assert!(tx.is_some());
        let tx = tx.unwrap();

        assert_eq!(
            tx.block_hash,
            Some(
                B256::from_str(
                    "0x88fadbb673928c61b9ede3694ae0589ac77ae38ec90a24a6e12e83f42f18c7e8"
                )
                .expect("couldn't parse data")
            )
        );
        assert_eq!(
            tx.block_number,
            Some(u64::from_str_radix("a74fde", 16).expect("couldn't parse data"))
        );
        assert_eq!(tx.hash, hash);
        assert_eq!(
            tx.from,
            Address::from_str("0x7d97fcdb98632a91be79d3122b4eb99c0c4223ee")
                .expect("couldn't parse data")
        );
        assert_eq!(
            tx.gas,
            U256::from_str_radix("30d40", 16).expect("couldn't parse data")
        );
        assert_eq!(tx.gas_price, 0x1e449a99b8);
        assert_eq!(
            tx.input,
            Bytes::from(hex::decode("a9059cbb000000000000000000000000e2c1e729e05f34c07d80083982ccd9154045dcc600000000000000000000000000000000000000000000000000000004a817c800").unwrap())
        );
        assert_eq!(
            tx.nonce,
            u64::from_str_radix("653b", 16).expect("couldn't parse data")
        );
        assert_eq!(
            tx.r,
            U256::from_str_radix(
                "eb56df45bd355e182fba854506bc73737df275af5a323d30f98db13fdf44393a",
                16
            )
            .expect("couldn't parse data")
        );
        assert_eq!(
            tx.s,
            U256::from_str_radix(
                "2c6efcd210cdc7b3d3191360f796ca84cab25a52ed8f72efff1652adaabc1c83",
                16
            )
            .expect("couldn't parse data")
        );
        assert_eq!(
            tx.to,
            Some(
                Address::from_str("dac17f958d2ee523a2206206994597c13d831ec7")
                    .expect("couldn't parse data")
            )
        );
        assert_eq!(
            tx.transaction_index,
            Some(u64::from_str_radix("88", 16).expect("couldn't parse data"))
        );
        assert_eq!(
            tx.v,
            u64::from_str_radix("1c", 16).expect("couldn't parse data")
        );
        assert_eq!(
            tx.value,
            U256::from_str_radix("0", 16).expect("couldn't parse data")
        );
    }

    #[tokio::test]
    async fn get_transaction_count_some() {
        let alchemy_url = get_alchemy_url();

        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");

        let transaction_count = TestRpcClient::new(&alchemy_url)
            .get_transaction_count(dai_address, Some(BlockSpec::Number(16220843)))
            .await
            .expect("should have succeeded");

        assert_eq!(transaction_count, U256::from(1));
    }

    #[tokio::test]
    async fn get_transaction_count_future_block() {
        let alchemy_url = get_alchemy_url();

        let missing_address = Address::from_str("0xffffffffffffffffffffffffffffffffffffffff")
            .expect("failed to parse address");

        let error = TestRpcClient::new(&alchemy_url)
            .get_transaction_count(missing_address, Some(BlockSpec::Number(MAX_BLOCK_NUMBER)))
            .await
            .expect_err("should have failed");

        assert!(matches!(error, RpcClientError::JsonRpcError { .. }));
    }

    #[tokio::test]
    async fn get_transaction_receipt_some() {
        let alchemy_url = get_alchemy_url();

        let hash =
            B256::from_str("0xc008e9f9bb92057dd0035496fbf4fb54f66b4b18b370928e46d6603933054d5a")
                .expect("failed to parse hash from string");

        let receipt = TestRpcClient::new(&alchemy_url)
            .get_transaction_receipt(hash)
            .await
            .expect("failed to get transaction by hash");

        assert!(receipt.is_some());
        let receipt = receipt.unwrap();

        assert_eq!(
            receipt.block_hash,
            B256::from_str("0x88fadbb673928c61b9ede3694ae0589ac77ae38ec90a24a6e12e83f42f18c7e8")
                .expect("couldn't parse data")
        );
        assert_eq!(receipt.block_number, 0xa74fde);
        assert_eq!(receipt.contract_address, None);
        assert_eq!(receipt.cumulative_gas_used, 0x56c81b);
        assert_eq!(receipt.effective_gas_price, Some(0x1e449a99b8));
        assert_eq!(
            receipt.from,
            Address::from_str("0x7d97fcdb98632a91be79d3122b4eb99c0c4223ee")
                .expect("couldn't parse data")
        );
        assert_eq!(
            receipt.gas_used,
            u64::from_str_radix("a0f9", 16).expect("couldn't parse data")
        );
        assert_eq!(receipt.logs.len(), 1);
        assert_eq!(receipt.state_root, None);
        assert_eq!(receipt.status, Some(true));
        assert_eq!(
            receipt.to,
            Some(
                Address::from_str("dac17f958d2ee523a2206206994597c13d831ec7")
                    .expect("couldn't parse data")
            )
        );
        assert_eq!(receipt.transaction_hash, hash);
        assert_eq!(receipt.transaction_index, 136);
        assert_eq!(receipt.transaction_type, Some(0));
    }

    #[tokio::test]
    async fn get_storage_at_some() {
        let alchemy_url = get_alchemy_url();

        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");

        let total_supply = TestRpcClient::new(&alchemy_url)
            .get_storage_at(
                dai_address,
                U256::from(1),
                Some(BlockSpec::Number(16220843)),
            )
            .await
            .expect("should have succeeded");

        assert_eq!(
            total_supply,
            Some(
                U256::from_str_radix(
                    "000000000000000000000000000000000000000010a596ae049e066d4991945c",
                    16
                )
                .expect("failed to parse storage location")
            )
        );
    }

    #[tokio::test]
    async fn get_storage_at_latest() {
        let alchemy_url = get_alchemy_url();

        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");

        let _total_supply = TestRpcClient::new(&alchemy_url)
            .get_storage_at(
                dai_address,
                U256::from_str_radix(
                    "0000000000000000000000000000000000000000000000000000000000000001",
                    16,
                )
                .expect("failed to parse storage location"),
                Some(BlockSpec::latest()),
            )
            .await
            .expect("should have succeeded");
    }

    #[tokio::test]
    async fn get_storage_at_future_block() {
        let alchemy_url = get_alchemy_url();

        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");

        let error = TestRpcClient::new(&alchemy_url)
            .get_storage_at(
                dai_address,
                U256::from(1),
                Some(BlockSpec::Number(MAX_BLOCK_NUMBER)),
            )
            .await
            .expect_err("should have failed");

        assert!(matches!(error, RpcClientError::JsonRpcError { .. }));
    }

    #[tokio::test]
    async fn network_id_success() {
        let alchemy_url = get_alchemy_url();

        let version = TestRpcClient::new(&alchemy_url)
            .network_id()
            .await
            .expect("should have succeeded");

        assert_eq!(version, 1);
    }

    #[tokio::test]
    async fn stores_result_in_cache() {
        let alchemy_url = get_alchemy_url();
        let client = TestRpcClient::new(&alchemy_url);
        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");

        let total_supply = client
            .get_storage_at(
                dai_address,
                U256::from(1),
                Some(BlockSpec::Number(16220843)),
            )
            .await
            .expect("should have succeeded");

        let cached_files = client.files_in_cache();
        assert_eq!(cached_files.len(), 1);

        let mut file = File::open(&cached_files[0]).expect("failed to open file");
        let cached_result: Option<U256> =
            serde_json::from_reader(&mut file).expect("failed to parse");

        assert_eq!(total_supply, cached_result);
    }

    #[tokio::test]
    async fn handles_invalid_type_in_cache_single_call() {
        let alchemy_url = get_alchemy_url();
        let client = TestRpcClient::new(&alchemy_url);
        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");

        client
            .get_storage_at(
                dai_address,
                U256::from(1),
                Some(BlockSpec::Number(16220843)),
            )
            .await
            .expect("should have succeeded");

        // Write some valid JSON, but invalid U256
        tokio::fs::write(&client.files_in_cache()[0], "\"not-hex\"")
            .await
            .unwrap();

        client
            .get_storage_at(
                dai_address,
                U256::from(1),
                Some(BlockSpec::Number(16220843)),
            )
            .await
            .expect("should have succeeded");
    }

    #[tokio::test]
    async fn fee_history() {
        let alchemy_url = get_alchemy_url();

        let _fee_history = TestRpcClient::new(&alchemy_url)
            .fee_history(
                /* block count */ 1,
                BlockSpec::latest(),
                /* reward percentiles */ vec![],
            )
            .await
            .expect("should have succeeded");
    }
}
