mod gas;
mod transactions;

use anyhow::Context;
use edr_chain_l1::L1ChainSpec;
use edr_provider::ProviderRequest;

#[test]
fn deserialize_single_request() -> anyhow::Result<()> {
    let json = r#"{
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": ["0x407d73d8a49eeb85d32cf465507dd71d507100c1", "latest"],
            "id": 1
        }"#;
    let request: ProviderRequest<L1ChainSpec> = serde_json::from_str(json)?;
    assert!(matches!(request, ProviderRequest::Single(..)));
    Ok(())
}

#[test]
fn deserialize_batch_request() -> anyhow::Result<()> {
    let json = r#"[
            {
                "jsonrpc": "2.0",
                "method": "eth_blockNumber",
                "params": [],
                "id": 1
            },
            {
                "jsonrpc": "2.0",
                "method": "eth_getTransactionByHash",
                "params": ["0x3f07a9c83155594c000642e7d60e8a8a00038d03e9849171a05ed0e2d47acbb3"],
                "id": 2
            }
        ]"#;
    let request: ProviderRequest<L1ChainSpec> = serde_json::from_str(json)?;
    assert!(matches!(request, ProviderRequest::Batch(_)));
    Ok(())
}

#[test]
fn deserialize_string_instead_of_request() -> anyhow::Result<()> {
    let s = "foo";
    let json = format!(r#""{s}""#);

    let result: Result<ProviderRequest<L1ChainSpec>, _> = serde_json::from_str(&json);

    let error_message = result.err().context("result is error")?.to_string();
    assert!(error_message.contains(s));

    Ok(())
}
