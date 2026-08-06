#![cfg(feature = "test-utils")]

//! EIP-7778: Block Gas Accounting without Refunds.
//! see <https://eips.ethereum.org/EIPS/eip-7778>
//
//! From Amsterdam onward, a block's `gas_used` counts gas before refunds, while
//! transaction receipts keep counting gas after refunds (unchanged). So the
//! block header's `gas_used` no longer matches the last receipt's
//! `cumulative_gas_used`: it is greater whenever the block contains refunds.

use std::sync::Arc;

use edr_chain_l1::{
    rpc::{block::L1RpcBlock, receipt::L1RpcTransactionReceipt, TransactionRequest},
    L1ChainSpec,
};
use edr_eth::PreEip1898BlockSpec;
use edr_primitives::{address, bytes, Address, Bytecode, Bytes, B256, U256};
use edr_provider::{
    test_utils::{create_test_config, one_ether, set_genesis_state_with_owned_accounts},
    time::CurrentTime,
    AccountOverride, MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_state_api::{EvmStorage, EvmStorageSlot};
use edr_test_utils::secret_key::secret_key_from_str;
use parking_lot::RwLock;
use tokio::runtime;

const CHAIN_ID: u64 = 0x7a69;

/// The first Hardhat account (owner of `SECRET_KEYS[0]`).
const CALLER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

/// Address of the storage-writer contract seeded into the genesis state.
const CONTRACT: Address = address!("0x000000000000000000000000000000000000c0de");

/// Deployed code `SSTORE(calldataload(0), calldataload(32))`: calling it with
/// `[slot(32) || value(32)]` writes `value` to storage `slot`.
const STORAGE_WRITER_CODE: Bytes = bytes!("0x6020356000355500");

/// Storage slot seeded into [`CONTRACT`] and cleared by the refunding
/// transaction.
const SLOT: u64 = 1;

/// Genesis storage for [`CONTRACT`]: [`SLOT`] set to a non-zero value, so that
/// clearing it to zero qualifies for a refund.
fn seeded_storage() -> EvmStorage {
    std::iter::once((U256::from(SLOT), EvmStorageSlot::new(U256::from(1), 0))).collect()
}

fn new_provider(hardfork: edr_chain_l1::Hardfork) -> anyhow::Result<Provider<L1ChainSpec>> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config();
    set_genesis_state_with_owned_accounts(&mut config, vec![secret_key], one_ether());
    config.chain_id = CHAIN_ID;
    config.hardfork = hardfork;

    // Seed the storage-writer contract with a non-zero slot to clear.
    config.genesis_state.insert(
        CONTRACT,
        AccountOverride {
            code: Some(Bytecode::new_raw(STORAGE_WRITER_CODE)),
            storage: Some(seeded_storage()),
            ..AccountOverride::default()
        },
    );

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

/// Transaction clearing [`SLOT`] to zero. The storage-writer contract takes
/// `[slot(32) || value(32)]` calldata; a zero value clears the slot.
fn clear_slot() -> TransactionRequest {
    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(&U256::from(SLOT).to_be_bytes::<32>());
    data.extend_from_slice(&U256::ZERO.to_be_bytes::<32>());

    TransactionRequest {
        from: CALLER,
        to: Some(CONTRACT),
        data: Some(data.into()),
        ..TransactionRequest::default()
    }
}

fn send_transaction(
    provider: &Provider<L1ChainSpec>,
    request: TransactionRequest,
) -> anyhow::Result<B256> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(request),
    ))?;

    Ok(serde_json::from_value(response.result)?)
}

fn transaction_receipt(
    provider: &Provider<L1ChainSpec>,
    transaction_hash: B256,
) -> anyhow::Result<L1RpcTransactionReceipt> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionReceipt(transaction_hash),
    ))?;

    let receipt: Option<L1RpcTransactionReceipt> = serde_json::from_value(response.result)?;
    receipt.ok_or_else(|| anyhow::anyhow!("receipt should exist"))
}

fn latest_block(provider: &Provider<L1ChainSpec>) -> anyhow::Result<L1RpcBlock<B256>> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    Ok(serde_json::from_value(response.result)?)
}

/// Clears the seeded slot, generating an SSTORE refund. With automining the
/// transaction is mined into its own block, which becomes the latest block.
fn send_refund_transaction(provider: &Provider<L1ChainSpec>) -> anyhow::Result<()> {
    send_transaction(provider, clear_slot())?;
    Ok(())
}

/// Before Amsterdam, a block's `gas_used` equals the last transaction receipt's
/// `cumulative_gas_used` even when the block's transactions clear storage slots
/// to zero: both track gas after refunds.
#[tokio::test(flavor = "multi_thread")]
async fn block_gas_used_matches_last_tx_cumulative_before_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::OSAKA)?;

    send_refund_transaction(&provider)?;

    let block = latest_block(&provider)?;
    let last_transaction = *block
        .transactions
        .last()
        .expect("block should contain transactions");
    let last_cumulative_gas_used =
        transaction_receipt(&provider, last_transaction)?.cumulative_gas_used;

    assert_eq!(
        block.gas_used, last_cumulative_gas_used,
        "before Amsterdam the block gas_used must equal the last receipt's cumulative_gas_used"
    );

    Ok(())
}

/// On Amsterdam (EIP-7778), the block's `gas_used` counts gas before refunds
/// while the receipt's `cumulative_gas_used` stays after refunds, so the block
/// `gas_used` is greater than the last receipt's `cumulative_gas_used`.
#[tokio::test(flavor = "multi_thread")]
async fn block_gas_used_excludes_refunds_on_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::AMSTERDAM)?;

    send_refund_transaction(&provider)?;

    let block = latest_block(&provider)?;
    let last_transaction = *block
        .transactions
        .last()
        .expect("block should contain transactions");
    let last_cumulative_gas_used =
        transaction_receipt(&provider, last_transaction)?.cumulative_gas_used;

    assert!(
        block.gas_used > last_cumulative_gas_used,
        "on Amsterdam the block gas_used (before refunds) must exceed the last receipt's \
         cumulative_gas_used (after refunds): {} vs {last_cumulative_gas_used}",
        block.gas_used
    );

    Ok(())
}
