//! From Amsterdam onward (EIP-7976), the calldata floor cost rises from 10/40
//! to 64/64 gas per (zero/nonzero) byte.

use edr_chain_l1::rpc::TransactionRequest;
use edr_primitives::{address, bytes};

use crate::{
    common::provider::new_provider, integration::calldata_floor::assert_transaction_gas_usage,
};

/// A transfer to an EOA carrying 4 nonzero and 4 zero calldata bytes, so the
/// floor exceeds the intrinsic gas and determines the transaction's gas usage.
fn transaction_request() -> TransactionRequest {
    TransactionRequest {
        from: address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
        to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
        data: Some(bytes!("0x1111111100000000")),
        ..TransactionRequest::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn calldata_floor_increases_in_amsterdam() -> anyhow::Result<()> {
    // 21_000 + 10 * (4 zero + 4 * 4 nonzero) = 21_200
    let osaka_provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;
    assert_transaction_gas_usage(&osaka_provider, transaction_request(), 21_200);

    // 21_000 + 16 * 4 * (4 zero + 4 nonzero) = 21_512
    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;
    assert_transaction_gas_usage(&amsterdam_provider, transaction_request(), 21_512);

    Ok(())
}
