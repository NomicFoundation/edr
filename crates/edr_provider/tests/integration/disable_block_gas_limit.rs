#![cfg(feature = "test-utils")]

use std::num::NonZeroU64;

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_defaults::SECRET_KEYS;
use edr_eth::{BlockSpec, PreEip1898BlockSpec};
use edr_mem_pool::MemPoolAddTransactionError;
use edr_primitives::{address, Address, Bytes, U256};
use edr_provider::{
    MethodInvocation, Provider, ProviderError, ProviderErrorForChainSpec, ProviderRequest,
    ResponseWithCallTraces,
};
use edr_test_utils::secret_key::secret_key_to_address;

use crate::common::{
    bytecode::{
        opcode::{DUP1, GASLIMIT, SSTORE},
        BytecodeBuilder,
    },
    provider::{
        block_gas_limit, new_provider_with_config, send_transaction as send_transaction_request,
    },
};

const BLOCK_GAS_LIMIT: u64 = 30_000_000;
const EXCEEDS_BLOCK_GAS_LIMIT: u64 = BLOCK_GAS_LIMIT + 1;
/// Stores `GASLIMIT` in slot 0 and returns it as a 32-byte word.
fn gas_limit_reporter_code() -> Bytes {
    let mut code = BytecodeBuilder::default();
    code.opcode(GASLIMIT)
        .opcode(DUP1)
        .push1(0)
        .opcode(SSTORE)
        .return_top_word();
    code.runtime()
}
const GAS_LIMIT_REPORTER_ADDRESS: Address = address!("0x1000000000000000000000000000000000000001");

fn new_provider(
    auto_mine: bool,
    block_gas_limit: Option<NonZeroU64>,
) -> anyhow::Result<Provider<L1ChainSpec>> {
    new_provider_with_config(|config| {
        config.mining.block_gas_limit = block_gas_limit;
        config.mining.auto_mine = auto_mine;
    })
}

fn send_transaction(
    provider: &Provider<L1ChainSpec>,
    gas_limit: u64,
) -> Result<ResponseWithCallTraces, ProviderErrorForChainSpec<L1ChainSpec>> {
    let caller = secret_key_to_address(SECRET_KEYS[0])?;
    let transaction = TransactionRequest {
        from: caller,
        to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
        gas: Some(gas_limit),
        ..TransactionRequest::default()
    };

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(transaction),
    ))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_transaction_with_auto_mine_exceeds_block_gas_limit() -> anyhow::Result<()> {
    let block_gas_limit = NonZeroU64::new(BLOCK_GAS_LIMIT).expect("non-zero");
    let provider = new_provider(true, Some(block_gas_limit))?;

    let result = send_transaction(&provider, EXCEEDS_BLOCK_GAS_LIMIT);

    assert!(
        matches!(
            &result,
            Err(ProviderError::MemPoolAddTransaction(
                MemPoolAddTransactionError::ExceedsBlockGasLimit {
                    block_gas_limit: limit,
                    transaction_gas_limit: EXCEEDS_BLOCK_GAS_LIMIT,
                }
            )) if limit.get() == BLOCK_GAS_LIMIT
        ),
        "{result:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_transaction_with_auto_mine_respects_disabled_block_gas_limit(
) -> anyhow::Result<()> {
    let provider = new_provider(true, None)?;

    send_transaction(&provider, EXCEEDS_BLOCK_GAS_LIMIT)?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_transaction_without_auto_mine_exceeds_block_gas_limit() -> anyhow::Result<()> {
    let block_gas_limit = NonZeroU64::new(BLOCK_GAS_LIMIT).expect("non-zero");
    let provider = new_provider(false, Some(block_gas_limit))?;

    let result = send_transaction(&provider, EXCEEDS_BLOCK_GAS_LIMIT);

    assert!(
        matches!(
            &result,
            Err(ProviderError::MemPoolAddTransaction(
                MemPoolAddTransactionError::ExceedsBlockGasLimit {
                    block_gas_limit: limit,
                    transaction_gas_limit: EXCEEDS_BLOCK_GAS_LIMIT,
                }
            )) if limit.get() == BLOCK_GAS_LIMIT
        ),
        "{result:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_transaction_without_auto_mine_respects_disabled_block_gas_limit(
) -> anyhow::Result<()> {
    let provider = new_provider(false, None)?;

    send_transaction(&provider, EXCEEDS_BLOCK_GAS_LIMIT)?;

    Ok(())
}

/// With the block gas limit disabled, a block takes its parent's gas limit.
/// `GASLIMIT` returns that value in a mined transaction and in a pending call.
#[tokio::test(flavor = "multi_thread")]
async fn gas_limit_opcode_returns_parent_gas_limit_with_disabled_block_gas_limit(
) -> anyhow::Result<()> {
    let provider = new_provider(true, None)?;
    provider.handle_request(ProviderRequest::with_single(MethodInvocation::SetCode(
        GAS_LIMIT_REPORTER_ADDRESS,
        gas_limit_reporter_code(),
    )))?;
    let parent_gas_limit = block_gas_limit(&provider, PreEip1898BlockSpec::latest())?;

    send_transaction_request(
        &provider,
        TransactionRequest {
            from: secret_key_to_address(SECRET_KEYS[0])?,
            to: Some(GAS_LIMIT_REPORTER_ADDRESS),
            ..TransactionRequest::default()
        },
    )?;

    assert_eq!(
        block_gas_limit(&provider, PreEip1898BlockSpec::latest())?,
        parent_gas_limit
    );

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetStorageAt(GAS_LIMIT_REPORTER_ADDRESS, U256::ZERO, None),
    ))?;
    let mined_gas_limit: U256 = response.deserialize_result()?;
    assert_eq!(mined_gas_limit, U256::from(parent_gas_limit));

    let response =
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            L1CallRequest {
                to: Some(GAS_LIMIT_REPORTER_ADDRESS),
                ..L1CallRequest::default()
            },
            Some(BlockSpec::pending()),
            None,
        )))?;
    let output: Bytes = response.deserialize_result()?;
    let pending_gas_limit = U256::try_from_be_slice(&output)
        .ok_or_else(|| anyhow::anyhow!("output should fit in a 32-byte word"))?;
    assert_eq!(pending_gas_limit, U256::from(parent_gas_limit));

    Ok(())
}
