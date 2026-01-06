//! Helper functions for environment variables

fn get_non_empty_env_var_or_panic(name: &'static str) -> String {
    let result = std::env::var_os(name)
        .unwrap_or_else(|| panic!("{name} environment variable not defined"))
        .into_string()
        .expect("Couldn't convert OsString into a String");
    if result.is_empty() {
        panic!("{name} environment variable is empty")
    } else {
        result
    }
}

/// This module exposes a provider-agnostic interface to obtain JSON-RPC
/// provider URLs for the different chains used across the codebase.
pub mod json_rpc_url_provider {
    use crate::env::get_non_empty_env_var_or_panic;

    /// Returns Alchemy JSON RPC provider URL from the environment variable.
    ///
    /// # Panics
    ///
    /// Panics if the `ALCHEMY_URL` environment variable is not defined or is
    /// empty.
    fn raw_eth_mainnet_alchemy_url() -> String {
        get_non_empty_env_var_or_panic("ALCHEMY_URL")
    }

    pub fn ethereum_mainnet() -> String {
        raw_eth_mainnet_alchemy_url()
    }
    pub fn ethereum_sepolia() -> String {
        raw_eth_mainnet_alchemy_url().replace("mainnet", "sepolia")
    }

    pub fn op_mainnet() -> String {
        raw_eth_mainnet_alchemy_url().replace("eth-", "opt-")
    }
    pub fn op_sepolia() -> String {
        raw_eth_mainnet_alchemy_url().replace("eth-mainnet", "opt-sepolia")
    }

    pub fn base_mainnet() -> String {
        raw_eth_mainnet_alchemy_url().replace("eth-", "base-")
    }
    pub fn base_sepolia() -> String {
        raw_eth_mainnet_alchemy_url().replace("eth-mainnet", "base-sepolia")
    }

    pub fn arbitrum_mainnet() -> String {
        raw_eth_mainnet_alchemy_url().replace("eth-", "arb-")
    }

    pub fn avalanche_mainnet() -> String {
        raw_eth_mainnet_alchemy_url().replace("eth-", "avax-")
    }

    pub fn avalanche_fuji() -> String {
        raw_eth_mainnet_alchemy_url().replace("eth-", "avax-fuji")
    }

    pub fn polygon_mainnet() -> String {
        raw_eth_mainnet_alchemy_url().replace("eth-", "polygon-")
    }
}
