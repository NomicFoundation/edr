#![cfg(feature = "test-utils")]

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    InvalidTransaction, L1ChainSpec,
};
use edr_chain_spec_evm::TransactionError;
use edr_defaults::SECRET_KEYS;
use edr_eip7825::OSAKA_TRANSACTION_GAS_CAP;
use edr_mem_pool::MemPoolAddTransactionError;
use edr_primitives::{address, Address};
use edr_provider::{
    config::ConfigOption, MethodInvocation, Provider, ProviderError, ProviderErrorForChainSpec,
    ProviderRequest, ResponseWithCallTraces,
};
use edr_test_utils::secret_key::secret_key_to_address;

use crate::common::{
    gas_reporter::{call_without_gas, deploy_gas_reporter, send_transaction_without_gas},
    provider::{new_provider_with_config, DEFAULT_TRANSACTION_GAS_LIMIT_ABOVE_OSAKA_CAP},
};

const CUSTOM_TRANSACTION_GAS_CAP: u64 = 50_000;
const EXCEEDS_TRANSACTION_GAS_LIMIT: u64 = CUSTOM_TRANSACTION_GAS_CAP + 1;
/// Above `OSAKA_TRANSACTION_GAS_CAP` (2^24), below the default block gas limit.
const EXCEEDS_OSAKA_CAP: u64 = 20_000_000;
const RECIPIENT: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

fn new_provider(
    auto_mine: bool,
    transaction_gas_cap: Option<u64>,
) -> anyhow::Result<Provider<L1ChainSpec>> {
    new_provider_with_config(|config| {
        config.hardfork = edr_chain_l1::Hardfork::Osaka;
        config.transaction_gas_cap =
            transaction_gas_cap.map_or(ConfigOption::Disable, ConfigOption::Custom);
        config.mining.auto_mine = auto_mine;
    })
}

/// Provider leaving the transaction gas cap at its hardfork default.
fn new_provider_with_default_cap(
    hardfork: edr_chain_l1::Hardfork,
) -> anyhow::Result<Provider<L1ChainSpec>> {
    new_provider_with_config(|config| {
        config.hardfork = hardfork;
        config.transaction_gas_cap = ConfigOption::Default;
        config.mining.auto_mine = false;
    })
}

fn call_request(gas_limit: u64) -> anyhow::Result<L1CallRequest> {
    Ok(L1CallRequest {
        from: Some(secret_key_to_address(SECRET_KEYS[0])?),
        to: Some(RECIPIENT),
        gas: Some(gas_limit),
        ..L1CallRequest::default()
    })
}

fn send_transaction(
    provider: &Provider<L1ChainSpec>,
    gas_limit: u64,
) -> Result<ResponseWithCallTraces, ProviderErrorForChainSpec<L1ChainSpec>> {
    let caller = secret_key_to_address(SECRET_KEYS[0])?;
    let transaction = TransactionRequest {
        from: caller,
        to: Some(RECIPIENT),
        gas: Some(gas_limit),
        ..TransactionRequest::default()
    };

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(transaction),
    ))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_call() -> anyhow::Result<()> {
    let provider = new_provider(false, Some(CUSTOM_TRANSACTION_GAS_CAP))?;

    let call = call_request(EXCEEDS_TRANSACTION_GAS_LIMIT)?;

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
                        cap: CUSTOM_TRANSACTION_GAS_CAP,
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
    let provider = new_provider(false, Some(CUSTOM_TRANSACTION_GAS_CAP))?;

    let call = call_request(EXCEEDS_TRANSACTION_GAS_LIMIT)?;

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
                        cap: CUSTOM_TRANSACTION_GAS_CAP,
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
    let provider = new_provider(true, Some(CUSTOM_TRANSACTION_GAS_CAP))?;

    let result = send_transaction(&provider, EXCEEDS_TRANSACTION_GAS_LIMIT);

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(ProviderError::MemPoolAddTransaction(
            MemPoolAddTransactionError::ExceedsTransactionGasCap {
                transaction_gas_cap: CUSTOM_TRANSACTION_GAS_CAP,
                transaction_gas_limit: EXCEEDS_TRANSACTION_GAS_LIMIT
            }
        ))
    ));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_transaction_exceeds_transaction_cap_without_auto_mine() -> anyhow::Result<()> {
    let provider = new_provider(false, Some(CUSTOM_TRANSACTION_GAS_CAP))?;

    let result = send_transaction(&provider, EXCEEDS_TRANSACTION_GAS_LIMIT);

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(ProviderError::MemPoolAddTransaction(
            MemPoolAddTransactionError::ExceedsTransactionGasCap {
                transaction_gas_cap: CUSTOM_TRANSACTION_GAS_CAP,
                transaction_gas_limit: EXCEEDS_TRANSACTION_GAS_LIMIT
            }
        ))
    ));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_disable_transaction_gas_cap_accepts_excess_gas() -> anyhow::Result<()> {
    // EIP-7825 caps transaction gas at `OSAKA_TRANSACTION_GAS_CAP` (2^24 = ~16.7M)
    // on Osaka. With the cap disabled, a transaction whose gas exceeds the cap
    // should still be accepted. We use a value below the default block gas
    // limit (30M) to keep this test focused on the transaction gas cap.
    let provider = new_provider(false, None)?;

    send_transaction(&provider, EXCEEDS_OSAKA_CAP)?;

    Ok(())
}

/// The default cap resolves to `OSAKA_TRANSACTION_GAS_CAP` from Osaka on.
#[tokio::test(flavor = "multi_thread")]
async fn test_default_cap_rejects_excess_gas_on_osaka() -> anyhow::Result<()> {
    let provider = new_provider_with_default_cap(edr_chain_l1::Hardfork::Osaka)?;

    let result = send_transaction(&provider, EXCEEDS_OSAKA_CAP);

    assert!(
        matches!(
            result,
            Err(ProviderError::MemPoolAddTransaction(
                MemPoolAddTransactionError::ExceedsTransactionGasCap {
                    transaction_gas_cap: OSAKA_TRANSACTION_GAS_CAP,
                    transaction_gas_limit: EXCEEDS_OSAKA_CAP
                }
            ))
        ),
        "{result:?}"
    );

    Ok(())
}

/// Before Osaka there is no cap to resolve, so any gas limit within the block
/// gas limit is accepted.
#[tokio::test(flavor = "multi_thread")]
async fn test_default_cap_is_absent_before_osaka() -> anyhow::Result<()> {
    let provider = new_provider_with_default_cap(edr_chain_l1::Hardfork::Prague)?;

    send_transaction(&provider, EXCEEDS_OSAKA_CAP)?;

    Ok(())
}

/// The Osaka cap bounds a configured default above it.
#[tokio::test(flavor = "multi_thread")]
async fn call_and_transaction_without_gas_cap_configured_default_on_osaka_provider(
) -> anyhow::Result<()> {
    let provider = new_provider_with_config(|config| {
        config.hardfork = edr_chain_l1::Hardfork::Osaka;
        config.transaction_gas_cap = ConfigOption::Default;
        config.default_transaction_gas_limit = DEFAULT_TRANSACTION_GAS_LIMIT_ABOVE_OSAKA_CAP;
    })?;
    deploy_gas_reporter(&provider)?;

    assert_eq!(
        call_without_gas(&provider, None)?,
        OSAKA_TRANSACTION_GAS_CAP
    );
    assert_eq!(
        send_transaction_without_gas(&provider)?,
        OSAKA_TRANSACTION_GAS_CAP
    );

    Ok(())
}

#[cfg(feature = "test-remote")]
mod fork {
    use edr_eth::BlockSpec;
    use edr_provider::config::ProviderConfig;

    use super::*;
    use crate::common::provider::{
        mainnet_fork_config, new_provider_from_config, OSAKA_MAINNET_BLOCK_NUMBER,
        PRAGUE_MAINNET_BLOCK_NUMBER,
    };

    /// Creates an Osaka provider forking mainnet at a Prague block, with the
    /// default transaction gas cap. `customize` adjusts the config further.
    fn new_osaka_provider_forked_at_prague_block(
        customize: impl FnOnce(&mut ProviderConfig<edr_chain_l1::Hardfork>),
    ) -> anyhow::Result<Provider<L1ChainSpec>> {
        let mut config = mainnet_fork_config(PRAGUE_MAINNET_BLOCK_NUMBER);
        config.hardfork = edr_chain_l1::Hardfork::Osaka;
        config.transaction_gas_cap = ConfigOption::Default;
        customize(&mut config);

        new_provider_from_config(config)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_and_estimate_gas_at_prague_block_accept_gas_above_osaka_cap_on_osaka_provider(
    ) -> anyhow::Result<()> {
        let provider = new_osaka_provider_forked_at_prague_block(|_| {})?;
        let block_spec = BlockSpec::Number(PRAGUE_MAINNET_BLOCK_NUMBER);

        provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            call_request(EXCEEDS_OSAKA_CAP)?,
            Some(block_spec.clone()),
            None,
        )))?;
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
            call_request(EXCEEDS_OSAKA_CAP)?,
            Some(block_spec),
        )))?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pending_call_rejects_gas_above_osaka_cap_on_osaka_provider_forked_at_prague_block(
    ) -> anyhow::Result<()> {
        let provider = new_osaka_provider_forked_at_prague_block(|_| {})?;

        let result = provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            call_request(EXCEEDS_OSAKA_CAP)?,
            Some(BlockSpec::pending()),
            None,
        )));

        assert!(
            matches!(
                result,
                Err(ProviderError::RunTransaction(
                    TransactionError::InvalidTransaction(
                        InvalidTransaction::TxGasLimitGreaterThanCap {
                            cap: OSAKA_TRANSACTION_GAS_CAP,
                            gas_limit: EXCEEDS_OSAKA_CAP
                        }
                    )
                ))
            ),
            "{result:?}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pending_transaction_rejects_gas_above_osaka_cap_on_osaka_provider_forked_at_prague_block(
    ) -> anyhow::Result<()> {
        let provider =
            new_osaka_provider_forked_at_prague_block(|config| config.mining.auto_mine = false)?;

        let result = send_transaction(&provider, EXCEEDS_OSAKA_CAP);

        assert!(
            matches!(
                result,
                Err(ProviderError::MemPoolAddTransaction(
                    MemPoolAddTransactionError::ExceedsTransactionGasCap {
                        transaction_gas_cap: OSAKA_TRANSACTION_GAS_CAP,
                        transaction_gas_limit: EXCEEDS_OSAKA_CAP
                    }
                ))
            ),
            "{result:?}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_at_prague_block_rejects_gas_above_custom_cap_on_osaka_provider(
    ) -> anyhow::Result<()> {
        let provider = new_osaka_provider_forked_at_prague_block(|config| {
            config.transaction_gas_cap = ConfigOption::Custom(CUSTOM_TRANSACTION_GAS_CAP);
        })?;

        let result = provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            call_request(EXCEEDS_TRANSACTION_GAS_LIMIT)?,
            Some(BlockSpec::Number(PRAGUE_MAINNET_BLOCK_NUMBER)),
            None,
        )));

        assert!(
            matches!(
                result,
                Err(ProviderError::RunTransaction(
                    TransactionError::InvalidTransaction(
                        InvalidTransaction::TxGasLimitGreaterThanCap {
                            cap: CUSTOM_TRANSACTION_GAS_CAP,
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
    async fn call_without_gas_at_osaka_block_caps_configured_default_on_prague_provider(
    ) -> anyhow::Result<()> {
        let mut config = mainnet_fork_config(OSAKA_MAINNET_BLOCK_NUMBER);
        config.hardfork = edr_chain_l1::Hardfork::Prague;
        config.transaction_gas_cap = ConfigOption::Default;
        config.default_transaction_gas_limit = DEFAULT_TRANSACTION_GAS_LIMIT_ABOVE_OSAKA_CAP;
        let provider = new_provider_from_config(config)?;
        deploy_gas_reporter(&provider)?;

        let gas_limit = call_without_gas(
            &provider,
            Some(BlockSpec::Number(OSAKA_MAINNET_BLOCK_NUMBER)),
        )?;
        assert_eq!(gas_limit, OSAKA_TRANSACTION_GAS_CAP);

        Ok(())
    }
}
