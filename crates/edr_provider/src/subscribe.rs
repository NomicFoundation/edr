use dyn_clone::DynClone;
use edr_eth::{filter::LogOutput, B256, U256};
use edr_evm::{blockchain::BlockchainError, chain_spec::L1ChainSpec, BlockAndTotalDifficulty};

/// Subscription event.
#[derive(Clone, Debug)]
pub struct SubscriptionEvent {
    pub filter_id: U256,
    pub result: SubscriptionEventData,
}

/// Subscription event data.
#[derive(Clone, Debug)]
pub enum SubscriptionEventData {
    Logs(Vec<LogOutput>),
    NewHeads(BlockAndTotalDifficulty<L1ChainSpec, BlockchainError>),
    NewPendingTransactions(B256),
}

/// Supertrait for subscription callbacks.
pub trait SyncSubscriberCallback: Fn(SubscriptionEvent) + DynClone + Send + Sync {}

impl<F> SyncSubscriberCallback for F where F: Fn(SubscriptionEvent) + DynClone + Send + Sync {}

dyn_clone::clone_trait_object!(SyncSubscriberCallback);
