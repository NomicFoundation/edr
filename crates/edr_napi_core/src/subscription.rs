use std::sync::Arc;

use edr_block_api::BlockAndTotalDifficulty;
use edr_eth::filter::LogOutput;
use edr_primitives::{B256, U256};
use napi::bindgen_prelude::Unknown;

/// A chain-agnostic version of [`edr_provider::SubscriptionEvent`].
pub struct SubscriptionEvent {
    pub filter_id: U256,
    pub result: SubscriptionEventData,
}

impl SubscriptionEvent {
    pub fn new<BlockT, RpcBlockT, SignedTransactionT>(
        event: edr_provider::SubscriptionEvent<BlockT, SignedTransactionT>,
    ) -> Self
    where
        BlockT: ?Sized + 'static,
        RpcBlockT:
            From<BlockAndTotalDifficulty<Arc<BlockT>, SignedTransactionT>> + serde::Serialize,
        SignedTransactionT: 'static,
    {
        let edr_provider::SubscriptionEvent { filter_id, result } = event;

        Self {
            filter_id,
            result: SubscriptionEventData::new::<_, RpcBlockT, _>(result),
        }
    }
}

/// Type alias for a closure trait object that constructs a JavaScript value.
///
/// Since [`serde::Serialize`] cannot be used as a dynamic trait object, we are
/// using a `FnOnce` to wrap N-API's [`napi::Env::to_js_value`] conversion
/// logic.
///
/// An alternative would be to use `serde_json::Value` as an intermediate
/// representation, but that would require an additional conversion step:
///
/// 1. Convert the value to `serde_json::Value`.
/// 2. Send the `serde_json::Value` to the `ThreadsafeFunction`.
/// 3. Convert the `serde_json::Value` to a JavaScript value using
///    `napi::Env::to_js_value`.
pub type DynJsValueConstructor =
    dyn for<'env> FnOnce(&'env napi::Env) -> napi::Result<Unknown<'static>>;

/// A chain-agnostic version of [`edr_provider::SubscriptionEventData`].
pub enum SubscriptionEventData {
    Logs(Vec<LogOutput>),
    /// A function that converts a [`BlockAndTotalDifficulty`] to a JS value.
    NewHeads(Box<DynJsValueConstructor>),
    NewPendingTransactions(B256),
}

impl SubscriptionEventData {
    pub fn new<BlockT, RpcBlockT, SignedTransactionT>(
        data: edr_provider::SubscriptionEventData<BlockT, SignedTransactionT>,
    ) -> Self
    where
        BlockT: ?Sized + 'static,
        RpcBlockT:
            From<BlockAndTotalDifficulty<Arc<BlockT>, SignedTransactionT>> + serde::Serialize,
        SignedTransactionT: 'static,
    {
        match data {
            edr_provider::SubscriptionEventData::Logs(log_outputs) => Self::Logs(log_outputs),
            edr_provider::SubscriptionEventData::NewHeads(block_and_total_difficulty) => {
                let block_to_js_value_fn: Box<DynJsValueConstructor> =
                    Box::new(move |env: &napi::Env| {
                        let block = RpcBlockT::from(block_and_total_difficulty);

                        env.to_js_value(&block)
                            .map_err(|error| napi::Error::from_reason(error.to_string()))
                    });

                Self::NewHeads(block_to_js_value_fn)
            }
            edr_provider::SubscriptionEventData::NewPendingTransactions(fixed_bytes) => {
                Self::NewPendingTransactions(fixed_bytes)
            }
        }
    }
}
