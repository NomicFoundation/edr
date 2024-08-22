//! RPC API keys utilities.

use crate::env::get_alchemy_url;

/// Returns the next _mainnet_ rpc endpoint in inline
///
/// This will rotate all available rpc endpoints
pub fn http_rpc_endpoint() -> String {
    get_alchemy_url()
}

/// Returns endpoint that has access to archive state
pub fn http_archive_rpc_endpoint() -> String {
    get_alchemy_url()
}

/// Returns endpoint that has access to archive state
pub fn ws_archive_rpc_endpoint() -> String {
    get_alchemy_url().replace("https://", "wss://")
}
