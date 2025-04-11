#![cfg(feature = "test-remote")]

mod full_block;
mod hardfork_activation;
mod provider;
mod rpc;

mod op {
    pub fn mainnet_url() -> String {
        use edr_test_utils::env::get_alchemy_url;

        get_alchemy_url().replace("eth-", "opt-")
    }

    pub fn sepolia_url() -> String {
        mainnet_url().replace("mainnet", "sepolia")
    }
}

mod base {
    pub fn mainnet_url() -> String {
        use edr_test_utils::env::get_alchemy_url;

        get_alchemy_url().replace("eth-", "base-")
    }

    pub fn sepolia_url() -> String {
        mainnet_url().replace("mainnet", "sepolia")
    }
}
