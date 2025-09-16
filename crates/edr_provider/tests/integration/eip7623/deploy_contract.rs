use edr_chain_l1::rpc::{call::L1CallRequest, TransactionRequest};
use edr_eth::{address, bytes};

use super::new_provider;
use crate::integration::eip7623::assert_transaction_gas_usage;

fn call_request() -> L1CallRequest {
    let transaction_request = transaction_request();

    L1CallRequest {
        from: Some(transaction_request.from),
        data: transaction_request.data,
        ..L1CallRequest::default()
    }
}

fn transaction_request() -> TransactionRequest {
    TransactionRequest {
        from: address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
        data: Some(bytes!("0x600b380380600b5f395ff300")),
        ..TransactionRequest::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas() -> anyhow::Result<()> {
    let cancun_provider = new_provider(edr_chain_l1::Hardfork::CANCUN)?;
    assert_eq!(
        super::estimate_gas(&cancun_provider, call_request()),
        53_409
    );

    let prague_provider = new_provider(edr_chain_l1::Hardfork::PRAGUE)?;
    assert_eq!(
        super::estimate_gas(&prague_provider, call_request()),
        53_409
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_transaction() -> anyhow::Result<()> {
    let cancun_provider = new_provider(edr_chain_l1::Hardfork::CANCUN)?;
    assert_transaction_gas_usage(&cancun_provider, transaction_request(), 53_409);

    let prague_provider = new_provider(edr_chain_l1::Hardfork::PRAGUE)?;
    assert_transaction_gas_usage(&prague_provider, transaction_request(), 53_409);

    Ok(())
}
