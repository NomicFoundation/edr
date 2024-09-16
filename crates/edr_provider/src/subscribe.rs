use derive_where::derive_where;
use dyn_clone::DynClone;
use edr_eth::{filter::LogOutput, B256, U256};
use edr_evm::{blockchain::BlockchainError, chain_spec::EvmSpec, BlockAndTotalDifficulty};

/// Subscription event.
#[derive_where(Clone, Debug)]
pub struct SubscriptionEvent<ChainSpecT: EvmSpec> {
    pub filter_id: U256,
    pub result: SubscriptionEventData<ChainSpecT>,
}

/// Subscription event data.
#[derive_where(Clone, Debug)]
pub enum SubscriptionEventData<ChainSpecT: EvmSpec> {
    Logs(Vec<LogOutput>),
    NewHeads(BlockAndTotalDifficulty<ChainSpecT, BlockchainError<ChainSpecT>>),
    NewPendingTransactions(B256),
}

/// Supertrait for subscription callbacks.
pub trait SyncSubscriberCallback<ChainSpecT: EvmSpec>:
    Fn(SubscriptionEvent<ChainSpecT>) + DynClone + Send + Sync
{
}

impl<ChainSpecT: EvmSpec, F> SyncSubscriberCallback<ChainSpecT> for F where
    F: Fn(SubscriptionEvent<ChainSpecT>) + DynClone + Send + Sync
{
}

dyn_clone::clone_trait_object!(<ChainSpecT> SyncSubscriberCallback<ChainSpecT> where ChainSpecT: EvmSpec);
