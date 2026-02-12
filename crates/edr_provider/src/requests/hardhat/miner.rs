use edr_chain_spec::TransactionValidation;

use crate::{
    data::ProviderData, spec::SyncProviderSpec, time::TimeSinceEpoch, ProviderError,
    ProviderResultWithCallTraces,
};

pub fn handle_mine<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    number_of_blocks: Option<u64>,
    interval: Option<u64>,
) -> ProviderResultWithCallTraces<bool, ChainSpecT> {
    let number_of_blocks = number_of_blocks.unwrap_or(1);
    let interval = interval.unwrap_or(1);

    let mined_block_results = data.mine_and_commit_blocks(number_of_blocks, interval)?;

    data.logger_mut()
        .log_mined_block(&mined_block_results)
        .map_err(ProviderError::Logger)?;

    let include_call_traces = data.include_call_traces();
    let traces = mined_block_results
        .into_iter()
        .flat_map(|result| {
            result
                .transaction_inspector_data
                .into_iter()
                .zip(result.transaction_results)
                .filter_map(|(observed_data, transaction_result)| {
                    if include_call_traces.should_include(|| !transaction_result.is_success()) {
                        Some(observed_data.call_trace_arena)
                    } else {
                        None
                    }
                })
        })
        .collect();

    Ok((true, traces))
}
