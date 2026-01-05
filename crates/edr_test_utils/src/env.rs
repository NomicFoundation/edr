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

pub struct JsonRpcUrlProvider;

impl JsonRpcUrlProvider {
    /// Returns Alchemy JSON RPC provider URL from the environment variables.
    ///
    /// # Panics
    ///
    /// Panics if the environment variable is not defined, or if it is empty.
    fn base_alchemy_url() -> String {
        get_non_empty_env_var_or_panic("ALCHEMY_URL")
    }
    fn convert_mainnet_to_sepolia(alchemy_url: String) -> String {
        alchemy_url.replace("mainnet", "sepolia")
    }

    pub fn ethereum_mainnet() -> String {
        Self::base_alchemy_url()
    }
    pub fn ethereum_sepolia() -> String {
        Self::convert_mainnet_to_sepolia(Self::ethereum_mainnet())
    }

    pub fn op_mainnet() -> String {
        Self::base_alchemy_url().replace("eth-", "opt-")
    }
    pub fn op_sepolia() -> String {
        Self::convert_mainnet_to_sepolia(Self::op_mainnet())
    }

    pub fn base_mainnet() -> String {
        Self::base_alchemy_url().replace("eth-", "base-")
    }
    pub fn base_sepolia() -> String {
        Self::convert_mainnet_to_sepolia(Self::base_mainnet())
    }

    pub fn arbitrum_mainnet() -> String {
        Self::base_alchemy_url().replace("eth-", "arb-")
    }

    pub fn avalanche_mainnet() -> String {
        Self::base_alchemy_url().replace("eth-", "avax-")
    }

    pub fn avalanche_fuji() -> String {
        "https://api.avax-test.network/ext/bc/C/rpc".into()
    }

    pub fn polygon_mainnet() -> String {
        Self::base_alchemy_url().replace("eth-", "polygon-")
    }
}
