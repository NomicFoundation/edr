use std::sync::Arc;

use edr_eth::l1;
use edr_evm::hardfork::ChainOverride;
use edr_evm_spec::TransactionValidation;
use edr_provider::{
    test_utils::create_test_config_with_fork, time::CurrentTime, ForkConfig, NoopLogger, Provider,
    ProviderSpec, SyncProviderSpec,
};
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

#[allow(dead_code)]
// allow it to avoid clippy complaining when running with
// --features tracing,serde,std
pub(crate) fn get_chain_fork_provider<
    ChainSpecT: SyncProviderSpec<
            CurrentTime,
            BlockEnv: Default,
            Hardfork = l1::SpecId,
            SignedTransaction: Default
                                   + TransactionValidation<
                ValidationError: From<l1::InvalidTransaction> + PartialEq,
            >,
        > + ProviderSpec<CurrentTime>,
>(
    chain_id: u64,
    block_number: u64,
    chain_override: ChainOverride<l1::SpecId>,
    url: String,
) -> anyhow::Result<Provider<ChainSpecT>> {
    let logger = Box::new(NoopLogger::<ChainSpecT>::default());
    let subscriber = Box::new(|_event| {});

    let chain_overrides = [(chain_id, chain_override)].into_iter().collect();

    let mut config = create_test_config_with_fork(Some(ForkConfig {
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
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?)
}
