use edr_primitives::{Bytes, B256};
use sha3::{Digest, Keccak256};

use crate::{time::TimeSinceEpoch, ProviderErrorForChainSpec, ProviderSpec};

pub fn client_version() -> String {
    format!(
        "edr/{}/revm/{}",
        env!("CARGO_PKG_VERSION"),
        env!("REVM_VERSION"),
    )
}

pub fn handle_web3_client_version_request<
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>() -> Result<String, ProviderErrorForChainSpec<ChainSpecT>> {
    Ok(client_version())
}

pub fn handle_web3_sha3_request<
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    message: Bytes,
) -> Result<B256, ProviderErrorForChainSpec<ChainSpecT>> {
    let hash = Keccak256::digest(&message[..]);
    Ok(B256::from_slice(&hash[..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_version_field_layout() {
        // Hardhat extracts the EDR version from field 1:
        // `clientVersion.split("/")[1]`.
        assert!(client_version().starts_with(&format!("edr/{}/revm/", env!("CARGO_PKG_VERSION"))));
    }
}
