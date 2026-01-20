#![cfg(feature = "test-utils")]

use std::sync::Arc;

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    InvalidTransaction, L1ChainSpec,
};
use edr_chain_spec::EvmSpecId;
use edr_chain_spec_evm::TransactionError;
use edr_defaults::SECRET_KEYS;
use edr_mem_pool::MemPoolAddTransactionError;
use edr_primitives::address;
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderError, ProviderErrorForChainSpec, ProviderRequest, ResponseWithCallTraces,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::secret_key::secret_key_to_address;
use tokio::runtime;

const TRANSACTION_GAS_CAP: u64 = 50_000;
const EXCEEDS_TRANSACTION_GAS_LIMIT: u64 = TRANSACTION_GAS_CAP + 1;

fn new_provider(
    auto_mine: bool,
    transaction_gas_cap: u64,
) -> anyhow::Result<Provider<L1ChainSpec>> {
    let mut config = create_test_config();
    config.hardfork = EvmSpecId::OSAKA;
    config.transaction_gas_cap = Some(transaction_gas_cap);
    config.mining.auto_mine = auto_mine;

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
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
async fn test_call() -> anyhow::Result<()> {
    let provider = new_provider(false, TRANSACTION_GAS_CAP)?;

    let caller = secret_key_to_address(SECRET_KEYS[0])?;
    let call = L1CallRequest {
        from: Some(caller),
        to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
        gas: Some(EXCEEDS_TRANSACTION_GAS_LIMIT),
        ..L1CallRequest::default()
    };

    let result = provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
        call, None, None,
    )));

    assert!(result.is_err());
    assert!(
        matches!(
            result,
            Err(ProviderError::RunTransaction(
                TransactionError::InvalidTransaction(
                    InvalidTransaction::TxGasLimitGreaterThanCap {
                        cap: TRANSACTION_GAS_CAP,
                        gas_limit: EXCEEDS_TRANSACTION_GAS_LIMIT
                    }
                )
            ))
        ),
        "{result:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_estimate_gas() -> anyhow::Result<()> {
    let provider = new_provider(false, TRANSACTION_GAS_CAP)?;

    let caller = secret_key_to_address(SECRET_KEYS[0])?;
    let call = L1CallRequest {
        from: Some(caller),
        to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
        gas: Some(EXCEEDS_TRANSACTION_GAS_LIMIT),
        ..L1CallRequest::default()
    };

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::EstimateGas(call, None),
    ));

    assert!(result.is_err());
    assert!(
        matches!(
            result,
            Err(ProviderError::RunTransaction(
                TransactionError::InvalidTransaction(
                    InvalidTransaction::TxGasLimitGreaterThanCap {
                        cap: TRANSACTION_GAS_CAP,
                        gas_limit: EXCEEDS_TRANSACTION_GAS_LIMIT
                    }
                )
            ))
        ),
        "{result:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_transaction_exceeds_transaction_cap_with_auto_mine() -> anyhow::Result<()> {
    let provider = new_provider(true, TRANSACTION_GAS_CAP)?;

    let result = send_transaction(&provider, EXCEEDS_TRANSACTION_GAS_LIMIT);

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(ProviderError::MemPoolAddTransaction(
            MemPoolAddTransactionError::ExceedsTransactionGasCap {
                transaction_gas_cap: TRANSACTION_GAS_CAP,
                transaction_gas_limit: EXCEEDS_TRANSACTION_GAS_LIMIT
            }
        ))
    ));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_transaction_exceeds_transaction_cap_without_auto_mine() -> anyhow::Result<()> {
    let provider = new_provider(false, TRANSACTION_GAS_CAP)?;

    let result = send_transaction(&provider, EXCEEDS_TRANSACTION_GAS_LIMIT);

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(ProviderError::MemPoolAddTransaction(
            MemPoolAddTransactionError::ExceedsTransactionGasCap {
                transaction_gas_cap: TRANSACTION_GAS_CAP,
                transaction_gas_limit: EXCEEDS_TRANSACTION_GAS_LIMIT
            }
        ))
    ));

    Ok(())
}
