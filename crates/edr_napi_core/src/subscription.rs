use std::sync::Arc;

use edr_eth::{filter::LogOutput, B256, U256};
use edr_evm::BlockAndTotalDifficulty;
use napi::{
    threadsafe_function::{
        ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
    },
    JsFunction, JsUnknown,
};

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

/// Type alias for a function that converts a value to a JavaScript value.
///
/// This is used because [`serde::Serialize`] cannot be used as a dynamic trait
/// object.
///
/// An alternative would be to use `serde_json::Value` as an intermediate
/// representation, but that would require an additional conversion step.
pub type ToJsValueFn = dyn FnOnce(&napi::Env) -> napi::Result<JsUnknown>;

/// A chain-agnostic version of [`edr_provider::SubscriptionEventData`].
pub enum SubscriptionEventData {
    Logs(Vec<LogOutput>),
    /// A function that converts a [`BlockAndTotalDifficulty`] to a JS value.
    NewHeads(Box<ToJsValueFn>),
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
                let block_to_js_value_fn = Box::new(move |env: &napi::Env| {
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

#[derive(Clone)]
pub struct Callback {
    inner: ThreadsafeFunction<SubscriptionEvent, ErrorStrategy::Fatal>,
}

impl Callback {
    pub fn new(env: &napi::Env, subscription_event_callback: JsFunction) -> napi::Result<Self> {
        let mut callback = subscription_event_callback.create_threadsafe_function(
            0,
            |ctx: ThreadSafeCallContext<SubscriptionEvent>| {
                // SubscriptionEvent
                let mut event = ctx.env.create_object()?;

                ctx.env
                    .create_bigint_from_words(false, ctx.value.filter_id.as_limbs().to_vec())
                    .and_then(|filter_id| event.set_named_property("filterId", filter_id))?;

                let result = match ctx.value.result {
                    SubscriptionEventData::Logs(logs) => ctx.env.to_js_value(&logs),
                    SubscriptionEventData::NewHeads(block_to_js_value_fn) => {
                        block_to_js_value_fn(&ctx.env)
                    }
                    SubscriptionEventData::NewPendingTransactions(tx_hash) => {
                        ctx.env.to_js_value(&tx_hash)
                    }
                }?;

                event.set_named_property("result", result)?;

                Ok(vec![event])
            },
        )?;

        // Maintain a weak reference to the function to avoid blocking the event loop
        // from exiting.
        callback.unref(env)?;

        Ok(Self { inner: callback })
    }

    pub fn call(&self, event: SubscriptionEvent) {
        // This is blocking because it's important that the subscription events are
        // in-order
        self.inner.call(event, ThreadsafeFunctionCallMode::Blocking);
    }
}

/// Configuration for subscriptions.
pub struct Config {
    /// Callback to be called when a new event is received.
    pub subscription_callback: JsFunction,
}
