//! Proxy env vars are read by `reqwest` when a client is built and are
//! process-global, so this lives in its own test binary rather than in
//! `main.rs`: setting `HTTP_PROXY` here cannot leak into the other integration
//! tests.

use edr_chain_l1::L1ChainSpec;
use edr_rpc_eth::client::{EthRpcClient, EthRpcClientForChainSpec};
use tempfile::TempDir;

/// Regression test for <https://github.com/NomicFoundation/edr/issues/1762>:
/// requests to a loopback URL must not go through `HTTP_PROXY`.
#[tokio::test]
async fn loopback_fork_url_bypasses_http_proxy() {
    let mut proxy = mockito::Server::new_async().await;
    let mut node = mockito::Server::new_async().await;

    // `reqwest` reads the proxy env vars when the client is built below.
    // SAFETY: this test binary is single-test, so nothing reads the
    // environment concurrently.
    unsafe {
        std::env::remove_var("NO_PROXY");
        std::env::remove_var("no_proxy");
        std::env::set_var("HTTP_PROXY", proxy.url());
        std::env::set_var("http_proxy", proxy.url());
    }

    // Any request that reaches the proxy is a failure. A proxied HTTP request
    // carries the absolute target URL, so match on any path.
    let proxy_mock = proxy
        .mock("POST", mockito::Matcher::Any)
        .with_status(502)
        .expect(0)
        .create_async()
        .await;

    let node_mock = node
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"jsonrpc":"2.0","id":1,"result":"0x1"}"#)
        .expect(1)
        .create_async()
        .await;

    let cache_dir = TempDir::new().unwrap();
    let client: EthRpcClientForChainSpec<L1ChainSpec> =
        EthRpcClient::new(&node.url(), cache_dir.path().into(), None).expect("url ok");

    let chain_id = client
        .chain_id()
        .await
        .expect("request reaches the node directly");
    assert_eq!(chain_id, 1);

    tokio::join!(node_mock.assert_async(), proxy_mock.assert_async());
}
