use std::sync::Arc;

use edr_chain_spec::{EvmTransactionValidationError, TransactionValidation};
use tokio::{runtime, sync::Mutex};

use crate::{
    data::ProviderData, error::ProviderErrorForChainSpec, interval::IntervalMiner, requests,
    spec::SyncProviderSpec, time::TimeSinceEpoch, IntervalConfig,
};

pub fn handle_set_interval_mining<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default
                               + TransactionValidation<
            ValidationError: From<EvmTransactionValidationError> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: Arc<Mutex<ProviderData<ChainSpecT, TimerT>>>,
    interval_miner: &mut Option<IntervalMiner<ChainSpecT, TimerT>>,
    runtime: runtime::Handle,
    config: requests::IntervalConfig,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    let config: Option<IntervalConfig> = config.try_into()?;
    *interval_miner = config.map(|config| IntervalMiner::new(runtime, config, data.clone()));

    Ok(true)
}
