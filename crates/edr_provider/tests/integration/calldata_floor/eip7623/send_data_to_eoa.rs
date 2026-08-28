use edr_chain_l1::rpc::{call::L1CallRequest, TransactionRequest};
use edr_primitives::{address, bytes};

use crate::integration::calldata_floor::{self, assert_transaction_gas_usage, new_provider};

fn call_request() -> L1CallRequest {
    let transaction_request = transaction_request();

    L1CallRequest {
        from: Some(transaction_request.from),
        to: transaction_request.to,
        data: transaction_request.data,
        ..L1CallRequest::default()
    }
}

fn transaction_request() -> TransactionRequest {
    TransactionRequest {
        from: address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
        to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
        data: Some(bytes!("0x11")),
        ..TransactionRequest::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas() -> anyhow::Result<()> {
    let cancun_provider = new_provider(edr_chain_l1::Hardfork::Cancun)?;
    assert_eq!(
        calldata_floor::estimate_gas(&cancun_provider, call_request()),
        // NOTE: Our estimate differs from the real cost by 1 gas unit.
        21_017
    );

    let prague_provider = new_provider(edr_chain_l1::Hardfork::Prague)?;
    assert_eq!(
        calldata_floor::estimate_gas(&prague_provider, call_request()),
        21_040
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_transaction() -> anyhow::Result<()> {
    let cancun_provider = new_provider(edr_chain_l1::Hardfork::Cancun)?;
    assert_transaction_gas_usage(&cancun_provider, transaction_request(), 21_016);

    let prague_provider = new_provider(edr_chain_l1::Hardfork::Prague)?;
    assert_transaction_gas_usage(&prague_provider, transaction_request(), 21_040);

    Ok(())
}
