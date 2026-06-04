#![cfg(feature = "test-utils")]

use std::{num::NonZeroU64, sync::Arc};

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_defaults::SECRET_KEYS;
use edr_mem_pool::MemPoolAddTransactionError;
use edr_primitives::address;
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderError, ProviderErrorForChainSpec, ProviderRequest, ResponseWithCallTraces,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::secret_key::secret_key_to_address;
use parking_lot::RwLock;
use tokio::runtime;

const BLOCK_GAS_LIMIT: u64 = 30_000_000;
const EXCEEDS_BLOCK_GAS_LIMIT: u64 = BLOCK_GAS_LIMIT + 1;

fn new_provider(
    auto_mine: bool,
    block_gas_limit: Option<NonZeroU64>,
) -> anyhow::Result<Provider<L1ChainSpec>> {
    let mut config = create_test_config();
    config.mining.block_gas_limit = block_gas_limit;
    config.mining.auto_mine = auto_mine;

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
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
