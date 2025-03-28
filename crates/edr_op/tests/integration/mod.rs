mod full_block;
mod hardfork_activation;
mod provider;
mod rpc;

use edr_test_utils::env::get_alchemy_url;

fn mainnet_url() -> String {
    get_alchemy_url().replace("eth-", "opt-")
}

fn sepolia_url() -> String {
    mainnet_url().replace("mainnet", "sepolia")
}
