use core::fmt::Debug;

use edr_eth::{result::InvalidTransaction, transaction::TransactionValidation};
use edr_evm::{chain_spec::SyncChainSpec, trace::Trace};

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_interval_mine_request<
    ChainSpecT: SyncChainSpec<
        Block: Default,
        Hardfork: Debug,
        Transaction: Default
                         + TransactionValidation<
            ValidationError: From<InvalidTransaction> + PartialEq,
        >,
    >,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    data.interval_mine()
}

pub fn handle_mine<
    ChainSpecT: SyncChainSpec<
        Block: Default,
        Hardfork: Debug,
        Transaction: Default
                         + TransactionValidation<
            ValidationError: From<InvalidTransaction> + PartialEq,
        >,
    >,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
    number_of_blocks: Option<u64>,
    interval: Option<u64>,
) -> Result<(bool, Vec<Trace<ChainSpecT>>), ProviderError<ChainSpecT, LoggerErrorT>> {
    let number_of_blocks = number_of_blocks.unwrap_or(1);
    let interval = interval.unwrap_or(1);

    let mined_block_results = data.mine_and_commit_blocks(number_of_blocks, interval)?;

    let spec_id = data.evm_spec_id();
    data.logger_mut()
        .log_mined_block(spec_id, &mined_block_results)
        .map_err(ProviderError::Logger)?;

    let traces = mined_block_results
        .into_iter()
        .flat_map(|result| result.transaction_traces)
        .collect();

    Ok((true, traces))
}
