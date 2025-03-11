use edr_eth::{address, bytes, transaction::EthTransactionRequest, SpecId};
use edr_rpc_eth::CallRequest;

use super::new_provider;
use crate::integration::eip7623::assert_transaction_gas_usage;

fn call_request() -> CallRequest {
    let transaction_request = transaction_request();

    CallRequest {
        from: Some(transaction_request.from),
        to: transaction_request.to,
        data: transaction_request.data,
        ..CallRequest::default()
    }
}

fn transaction_request() -> EthTransactionRequest {
    EthTransactionRequest {
        from: address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
        to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
        data: Some(bytes!("0x11")),
        ..EthTransactionRequest::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas() -> anyhow::Result<()> {
    let cancun_provider = new_provider(SpecId::CANCUN)?;
    assert_eq!(
        super::estimate_gas(&cancun_provider, call_request()),
        // NOTE: Our estimate differs from the real cost by 1 gas unit.
        21_017
    );

    let prague_provider = new_provider(SpecId::PRAGUE)?;
    assert_eq!(
        super::estimate_gas(&prague_provider, call_request()),
        21_040
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_transaction() -> anyhow::Result<()> {
    let cancun_provider = new_provider(SpecId::CANCUN)?;
    assert_transaction_gas_usage(&cancun_provider, transaction_request(), 21_016);

    let prague_provider = new_provider(SpecId::PRAGUE)?;
    assert_transaction_gas_usage(&prague_provider, transaction_request(), 21_040);

    Ok(())
}
