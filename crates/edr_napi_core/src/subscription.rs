use edr_eth::B256;
use edr_provider::{time::CurrentTime, ProviderSpec};
use napi::{
    bindgen_prelude::BigInt,
    threadsafe_function::{
        ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
    },
    Env, JsFunction,
};
use napi_derive::napi;

#[derive(Clone)]
pub struct SubscriptionCallback<ChainSpecT: ProviderSpec<CurrentTime>> {
    inner: ThreadsafeFunction<edr_provider::SubscriptionEvent<ChainSpecT>, ErrorStrategy::Fatal>,
}

impl<ChainSpecT: ProviderSpec<CurrentTime>> SubscriptionCallback<ChainSpecT> {
    pub fn new(env: &Env, subscription_event_callback: JsFunction) -> napi::Result<Self> {
        let mut callback = subscription_event_callback.create_threadsafe_function(
            0,
            |ctx: ThreadSafeCallContext<edr_provider::SubscriptionEvent<ChainSpecT>>| {
                // SubscriptionEvent
                let mut event = ctx.env.create_object()?;

                ctx.env
                    .create_bigint_from_words(false, ctx.value.filter_id.as_limbs().to_vec())
                    .and_then(|filter_id| event.set_named_property("filterId", filter_id))?;

                let result = match ctx.value.result {
                    edr_provider::SubscriptionEventData::Logs(logs) => ctx.env.to_js_value(&logs),
                    edr_provider::SubscriptionEventData::NewHeads(block) => {
                        let block = ChainSpecT::RpcBlock::<B256>::from(block);
                        ctx.env.to_js_value(&block)
                    }
                    edr_provider::SubscriptionEventData::NewPendingTransactions(tx_hash) => {
                        ctx.env.to_js_value(&tx_hash)
                    }
                }?;

                event.set_named_property("result", result)?;

                Ok(vec![event])
            },
        )?;

        // Maintain a weak reference to the function to avoid the event loop from
        // exiting.
        callback.unref(env)?;

        Ok(Self { inner: callback })
    }

    pub fn call(&self, event: edr_provider::SubscriptionEvent<ChainSpecT>) {
        // This is blocking because it's important that the subscription events are
        // in-order
        self.inner.call(event, ThreadsafeFunctionCallMode::Blocking);
    }
}

/// Configuration for subscriptions.
#[napi(object)]
pub struct SubscriptionConfig {
    /// Callback to be called when a new event is received.
    #[napi(ts_type = "(event: SubscriptionEvent) => void")]
    pub subscription_callback: JsFunction,
}

#[napi(object)]
pub struct SubscriptionEvent {
    pub filter_id: BigInt,
    pub result: serde_json::Value,
}
