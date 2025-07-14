use std::{str::FromStr, sync::Arc};

use edr_chain_l1::L1ChainSpec;
use edr_eth::{account::AccountInfo, Address, U256};
use edr_evm::state::{RemoteState, State as _};
use edr_rpc_eth::client::EthRpcClient;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn basic_success() {
    let tempdir = tempfile::tempdir().expect("can create tempdir");

    let alchemy_url = std::env::var_os("ALCHEMY_URL")
        .expect("ALCHEMY_URL environment variable not defined")
        .into_string()
        .expect("couldn't convert OsString into a String");

    let rpc_client =
        EthRpcClient::<L1ChainSpec>::new(&alchemy_url, tempdir.path().to_path_buf(), None)
            .expect("url ok");

    let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
        .expect("failed to parse address");

    let runtime = runtime::Handle::current();

    let account_info: AccountInfo = RemoteState::new(runtime, Arc::new(rpc_client), 16643427)
        .basic(dai_address)
        .expect("should succeed")
        .unwrap();

    assert_eq!(account_info.balance, U256::from(0));
    assert_eq!(account_info.nonce, 1);
}
