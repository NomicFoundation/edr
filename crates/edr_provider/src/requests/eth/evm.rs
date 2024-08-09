use std::num::NonZeroU64;

use edr_eth::{block::BlockOptions, chain_spec::L1ChainSpec, U64};
use edr_evm::trace::Trace;

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError, Timestamp};

pub fn handle_increase_time_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    increment: Timestamp,
) -> Result<String, ProviderError> {
    let new_block_time = data.increase_block_time(increment.into());

    // This RPC call is an exception: it returns a number as a string decimal
    Ok(new_block_time.to_string())
}

pub fn handle_mine_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    timestamp: Option<Timestamp>,
) -> Result<(String, Vec<Trace<L1ChainSpec>>), ProviderError> {
    let mine_block_result = data.mine_and_commit_block(BlockOptions {
        timestamp: timestamp.map(Into::into),
        ..BlockOptions::default()
    })?;

    let traces = mine_block_result.transaction_traces.clone();

    let spec_id = data.spec_id();
    data.logger_mut()
        .log_mined_block(spec_id, &[mine_block_result])
        .map_err(ProviderError::Logger)?;

    let result = String::from("0");
    Ok((result, traces))
}

pub fn handle_revert_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    snapshot_id: U64,
) -> Result<bool, ProviderError> {
    Ok(data.revert_to_snapshot(snapshot_id.as_limbs()[0]))
}

pub fn handle_set_automine_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    automine: bool,
) -> Result<bool, ProviderError> {
    data.set_auto_mining(automine);

    Ok(true)
}

pub fn handle_set_block_gas_limit_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    gas_limit: U64,
) -> Result<bool, ProviderError> {
    let gas_limit = NonZeroU64::new(gas_limit.as_limbs()[0])
        .ok_or(ProviderError::SetBlockGasLimitMustBeGreaterThanZero)?;

    data.set_block_gas_limit(gas_limit)?;

    Ok(true)
}

pub fn handle_set_next_block_timestamp_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    timestamp: Timestamp,
) -> Result<String, ProviderError> {
    let new_timestamp = data.set_next_block_timestamp(timestamp.into())?;

    // This RPC call is an exception: it returns a number as a string decimal
    Ok(new_timestamp.to_string())
}

pub fn handle_snapshot_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
) -> Result<U64, ProviderError> {
    let snapshot_id = data.make_snapshot();

    Ok(U64::from(snapshot_id))
}
