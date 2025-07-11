use edr_eth::transaction::TransactionValidation;
use edr_evm::EvmInvalidTransaction;

use crate::{
    data::ProviderData, error::ProviderErrorForChainSpec, spec::SyncProviderSpec,
    time::TimeSinceEpoch, ProviderError, ProviderResultWithTraces,
};

pub fn handle_interval_mine_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<
            ValidationError: From<EvmInvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    data.interval_mine()
}

pub fn handle_mine<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<
            ValidationError: From<EvmInvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    number_of_blocks: Option<u64>,
    interval: Option<u64>,
) -> ProviderResultWithTraces<bool, ChainSpecT> {
    let number_of_blocks = number_of_blocks.unwrap_or(1);
    let interval = interval.unwrap_or(1);

    let mined_block_results = data.mine_and_commit_blocks(number_of_blocks, interval)?;

    let hardfork = data.hardfork();
    data.logger_mut()
        .log_mined_block(hardfork, &mined_block_results)
        .map_err(ProviderError::Logger)?;

    let traces = mined_block_results
        .into_iter()
        .flat_map(|result| result.transaction_traces)
        .collect();

    Ok((true, traces))
}
