use core::fmt::Debug;

use edr_eth::B256;
use edr_evm::chain_spec::SyncChainSpec;

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_drop_transaction<
    ChainSpecT: SyncChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
    transaction_hash: B256,
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
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
