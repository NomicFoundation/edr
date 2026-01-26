use std::sync::Arc;

use edr_chain_config::ChainOverride;
use edr_chain_spec::TransactionValidation;
use edr_provider::{
    test_utils::{create_test_config_with, MinimalProviderConfig},
    time::CurrentTime,
    ForkConfig, NoopLogger, Provider, ProviderSpec, SyncProviderSpec,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

#[allow(dead_code)]
// allow it to avoid clippy complaining when running with
// --features tracing,serde,std
pub(crate) fn get_chain_fork_provider<
    ChainSpecT: SyncProviderSpec<
            CurrentTime,
            Hardfork = edr_chain_l1::Hardfork,
            SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
        > + ProviderSpec<CurrentTime>,
>(
    chain_id: u64,
    block_number: u64,
    chain_override: ChainOverride<edr_chain_l1::Hardfork>,
    url: String,
) -> anyhow::Result<Provider<ChainSpecT>> {
    let logger = Box::new(NoopLogger::<ChainSpecT>::default());
    let subscriber = Box::new(|_event| {});

    let chain_overrides = [(chain_id, chain_override)].into_iter().collect();

    let mut config =
        create_test_config_with(MinimalProviderConfig::fork_with_accounts(ForkConfig {
            block_number: Some(block_number),
            cache_dir: edr_defaults::CACHE_DIR.into(),
            chain_overrides,
            http_headers: None,
            url,
        }));

    config.chain_id = chain_id;

    Ok(Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?)
}
