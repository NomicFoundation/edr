use edr_eth::{B256, Bytes};
use edr_evm::spec::RuntimeSpec;
use sha3::{Digest, Keccak256};

use crate::ProviderErrorForChainSpec;

pub fn client_version() -> String {
    format!(
        "edr/{}/revm/{}",
        env!("CARGO_PKG_VERSION"),
        env!("REVM_VERSION"),
    )
}

pub fn handle_web3_client_version_request<ChainSpecT: RuntimeSpec>()
-> Result<String, ProviderErrorForChainSpec<ChainSpecT>> {
    Ok(client_version())
}

pub fn handle_web3_sha3_request<ChainSpecT: RuntimeSpec>(
    message: Bytes,
) -> Result<B256, ProviderErrorForChainSpec<ChainSpecT>> {
    let hash = Keccak256::digest(&message[..]);
    Ok(B256::from_slice(&hash[..]))
}
