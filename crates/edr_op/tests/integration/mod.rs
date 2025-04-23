mod full_block;
mod hardfork_activation;
mod provider;
mod rpc;

#[cfg(feature = "test-remote")]
fn mainnet_url() -> String {
    use edr_test_utils::env::get_alchemy_url;

    get_alchemy_url().replace("eth-", "opt-")
}

#[cfg(feature = "test-remote")]
fn sepolia_url() -> String {
    mainnet_url().replace("mainnet", "sepolia")
}
