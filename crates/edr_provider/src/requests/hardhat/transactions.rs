use edr_primitives::B256;

use crate::{
    data::ProviderData, error::ProviderErrorForChainSpec, spec::SyncProviderSpec,
    time::TimeSinceEpoch, ProviderError,
};

pub fn handle_drop_transaction<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    transaction_hash: B256,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    let was_removed = data.remove_pending_transaction(&transaction_hash).is_some();
    if was_removed {
        return Ok(true);
    }

    let was_transaction_mined = data.transaction_receipt(&transaction_hash)?.is_some();
    if was_transaction_mined {
        Err(ProviderError::InvalidDropTransactionHash(transaction_hash))
    } else {
        Ok(false)
    }
}
