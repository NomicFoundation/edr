mod dynamic_base_fee_params;
mod full_block;
mod hardfork_activation;
mod holocene;
mod isthmus_operator_fee;
mod provider;
mod rpc;

#[cfg(feature = "test-remote")]
// TODO: remove now that's encapsulated
mod op {
    use edr_test_utils::env::JsonRpcUrlProvider;

    pub fn mainnet_url() -> String {
        JsonRpcUrlProvider::op_mainnet()
    }

    pub fn sepolia_url() -> String {
        JsonRpcUrlProvider::op_sepolia()
    }
}

// TODO: remove now that's encapsulated
#[cfg(feature = "test-remote")]
mod base {
    use edr_test_utils::env::JsonRpcUrlProvider;

    pub fn mainnet_url() -> String {
        JsonRpcUrlProvider::base_mainnet()
    }

    pub fn sepolia_url() -> String {
        JsonRpcUrlProvider::base_sepolia()
    }
}
