use std::num::NonZeroU64;

use edr_block_header::HeaderOverrides;
use edr_chain_spec::TransactionValidation;
use edr_primitives::U64;

use crate::{
    data::ProviderData,
    error::ProviderErrorForChainSpec,
    spec::{ProviderSpec, SyncProviderSpec},
    time::TimeSinceEpoch,
    ProviderError, ProviderResultWithCallTraces, Timestamp,
};

pub fn handle_increase_time_request<
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    increment: Timestamp,
) -> Result<String, ProviderErrorForChainSpec<ChainSpecT>> {
    let new_block_time = data.increase_block_time(increment.into());

    // This RPC call is an exception: it returns a number as a string decimal
    Ok(new_block_time.to_string())
}

pub fn handle_mine_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    timestamp: Option<Timestamp>,
) -> ProviderResultWithCallTraces<String, ChainSpecT> {
    let mine_block_result = data.mine_and_commit_block(HeaderOverrides {
        timestamp: timestamp.map(Into::into),
        ..HeaderOverrides::default()
    })?;

    let traces = mine_block_result.transaction_call_trace_arenas.clone();

    let hardfork = data.hardfork();
    data.logger_mut()
        .log_mined_block(hardfork, &[mine_block_result])
        .map_err(ProviderError::Logger)?;

    let result = String::from("0");
    Ok((result, traces))
}

pub fn handle_revert_request<ChainSpecT: ProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    snapshot_id: U64,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    Ok(data.revert_to_snapshot(snapshot_id.as_limbs()[0]))
}

pub fn handle_set_automine_request<
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    automine: bool,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    data.set_auto_mining(automine);

    Ok(true)
}

pub fn handle_set_block_gas_limit_request<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    gas_limit: U64,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    let gas_limit = NonZeroU64::new(gas_limit.as_limbs()[0])
        .ok_or(ProviderError::SetBlockGasLimitMustBeGreaterThanZero)?;

    data.set_block_gas_limit(gas_limit)?;

    Ok(true)
}

pub fn handle_set_next_block_timestamp_request<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    timestamp: Timestamp,
) -> Result<String, ProviderErrorForChainSpec<ChainSpecT>> {
    let new_timestamp = data.set_next_block_timestamp(timestamp.into())?;

    // This RPC call is an exception: it returns a number as a string decimal
    Ok(new_timestamp.to_string())
}

pub fn handle_snapshot_request<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
) -> Result<U64, ProviderErrorForChainSpec<ChainSpecT>> {
    let snapshot_id = data.make_snapshot();

    Ok(U64::from(snapshot_id))
}
