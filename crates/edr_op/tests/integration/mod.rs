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
    use edr_test_utils::env::json_rpc_url_provider;

    pub fn mainnet_url() -> String {
        json_rpc_url_provider::op_mainnet()
    }

    pub fn sepolia_url() -> String {
        json_rpc_url_provider::op_sepolia()
    }
}

// TODO: remove now that's encapsulated
#[cfg(feature = "test-remote")]
mod base {
    use edr_test_utils::env::json_rpc_url_provider;

    pub fn mainnet_url() -> String {
        json_rpc_url_provider::base_mainnet()
    }

    pub fn sepolia_url() -> String {
        json_rpc_url_provider::base_sepolia()
    }
}
