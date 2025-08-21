use std::sync::Arc;

use dyn_clone::DynClone;
use edr_eth::{filter::LogOutput, B256, U256};
use edr_evm::BlockAndTotalDifficulty;

/// Subscription event.
#[derive(Clone, Debug)]
pub struct SubscriptionEvent<BlockT: ?Sized, SignedTransactionT> {
    pub filter_id: U256,
    pub result: SubscriptionEventData<BlockT, SignedTransactionT>,
}

/// Subscription event data.
#[derive(Clone, Debug)]
pub enum SubscriptionEventData<BlockT: ?Sized, SignedTransactionT> {
    Logs(Vec<LogOutput>),
    NewHeads(BlockAndTotalDifficulty<Arc<BlockT>, SignedTransactionT>),
    NewPendingTransactions(B256),
}

/// Supertrait for subscription callbacks.
pub trait SyncSubscriberCallback<BlockT: ?Sized, SignedTransactionT>:
    Fn(SubscriptionEvent<BlockT, SignedTransactionT>) + DynClone + Send + Sync
{
}

impl<BlockT: ?Sized, SignedTransactionT, F> SyncSubscriberCallback<BlockT, SignedTransactionT> for F where
    F: Fn(SubscriptionEvent<BlockT, SignedTransactionT>) + DynClone + Send + Sync
{
}

dyn_clone::clone_trait_object!(<BlockT: ?Sized, SignedTransactionT> SyncSubscriberCallback<BlockT, SignedTransactionT>);
