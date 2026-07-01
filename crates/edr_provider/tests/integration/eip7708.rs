#![cfg(feature = "test-utils")]

//! EIP-7708: ETH transfers emit a log.
//! see <https://eips.ethereum.org/EIPS/eip-7708>
//
//! A value-bearing transfer emits a LOG3 from a system address mirroring the
//! ERC-20 `Transfer(address,address,uint256)` event.

use std::sync::Arc;

use edr_chain_l1::{
    rpc::{receipt::L1RpcTransactionReceipt, TransactionRequest},
    L1ChainSpec,
};
use edr_primitives::{address, eip7708, Address, Bytes, B256, U256};
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

const SENDER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const RECIPIENT: Address = address!("0x70997970C51812dc3A010C7d01b50e0d17dc79C8");

fn new_provider(hardfork: edr_chain_l1::Hardfork) -> anyhow::Result<Provider<L1ChainSpec>> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config();
    config.hardfork = hardfork;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    Ok(provider)
}

fn transfer_value(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    to: Address,
    value: U256,
) -> L1RpcTransactionReceipt {
    let request = TransactionRequest {
        from,
        to: Some(to),
        value: Some(value),
        ..TransactionRequest::default()
    };

    let response = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(request),
        ))
        .expect("eth_sendTransaction should succeed");

    let transaction_hash: B256 =
        serde_json::from_value(response.result).expect("response should be a transaction hash");

    let response = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::GetTransactionReceipt(transaction_hash),
        ))
        .expect("eth_getTransactionReceipt should succeed");

    let receipt: Option<L1RpcTransactionReceipt> =
        serde_json::from_value(response.result).expect("response should be a receipt");

    receipt.expect("receipt should exist")
}

#[tokio::test(flavor = "multi_thread")]
async fn emits_transfer_log_on_amsterdam_provider() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::AMSTERDAM)?;

    let value = U256::from(1000);
    let receipt = transfer_value(&provider, SENDER, RECIPIENT, value);

    let transfer_log = receipt
        .logs
        .iter()
        .find(|log| {
            log.address == eip7708::ETH_TRANSFER_LOG_ADDRESS
                && log.data.topics().first() == Some(&eip7708::ETH_TRANSFER_LOG_TOPIC)
        })
        .expect("an EIP-7708 ETH transfer log should be present in the receipt");

    let topics = transfer_log.data.topics();
    assert_eq!(topics[1], B256::left_padding_from(SENDER.as_slice()));
    assert_eq!(topics[2], B256::left_padding_from(RECIPIENT.as_slice()));
    assert_eq!(
        transfer_log.data.data,
        Bytes::copy_from_slice(&value.to_be_bytes::<32>())
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn does_not_emit_transfer_log_before_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::OSAKA)?;

    let receipt = transfer_value(&provider, SENDER, RECIPIENT, U256::from(1000));

    let transfer_log = receipt.logs.iter().find(|log| {
        log.address == eip7708::ETH_TRANSFER_LOG_ADDRESS
            && log.data.topics().first() == Some(&eip7708::ETH_TRANSFER_LOG_TOPIC)
    });

    assert!(
        transfer_log.is_none(),
        "no EIP-7708 transfer log should be emitted before Amsterdam"
    );

    Ok(())
}
