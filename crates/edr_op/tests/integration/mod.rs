mod dynamic_base_fee_params;
mod full_block;
mod hardfork_activation;
mod holocene;
mod provider;
mod rpc;

#[cfg(feature = "test-remote")]
mod op {
    pub fn mainnet_url() -> String {
        use edr_test_utils::env::get_alchemy_url;

        get_alchemy_url().replace("eth-", "opt-")
    }

    pub fn sepolia_url() -> String {
        mainnet_url().replace("mainnet", "sepolia")
    }
}

#[cfg(feature = "test-remote")]
mod base {
    pub fn mainnet_url() -> String {
        use edr_test_utils::env::get_alchemy_url;

        get_alchemy_url().replace("eth-", "base-")
    }

    pub fn sepolia_url() -> String {
        mainnet_url().replace("mainnet", "sepolia")
    }
}
