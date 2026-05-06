use std::sync::Arc;

use edr_block_api::BlockAndTotalDifficulty;
use edr_eth::filter::LogOutput;
use edr_primitives::{B256, U256};
use edr_provider::{time::TimeSinceEpoch, ProviderSpec, SyncSubscriberCallback};
#[allow(deprecated)]
use napi::JsObject;
use napi::{
    bindgen_prelude::{BigInt, Function, Unknown},
    threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode},
};

pub fn subscriber_callback_for_chain_spec<
    ChainSpecT: ProviderSpec<TimerT, Block: 'static, SignedTransaction: 'static>,
    TimerT: Clone + TimeSinceEpoch,
>(
    subscription_callback: Callback,
) -> Box<dyn SyncSubscriberCallback<ChainSpecT::Block, ChainSpecT::SignedTransaction>> {
    Box::new(move |event| {
        let event = SubscriptionEvent::new::<
            ChainSpecT::Block,
            ChainSpecT::RpcBlock<B256>,
            ChainSpecT::SignedTransaction,
        >(event);

        subscription_callback.call(event);
    })
}

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
    dyn for<'env> FnOnce(&'env napi::Env) -> napi::Result<Unknown<'env>>;

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

#[allow(deprecated)]
type SubscriptionTsfn =
    ThreadsafeFunction<SubscriptionEvent, (), JsObject, napi::Status, false, true, 0>;

#[derive(Clone)]
pub struct Callback {
    inner: Arc<SubscriptionTsfn>,
}

impl Callback {
    #[allow(deprecated)]
    pub fn new(
        _env: &napi::Env,
        subscription_event_callback: Function<'_, JsObject, ()>,
    ) -> napi::Result<Self> {
        let callback = subscription_event_callback
            .build_threadsafe_function::<SubscriptionEvent>()
            .weak::<true>()
            .build_callback(|ctx: ThreadsafeCallContext<SubscriptionEvent>| {
                let env = ctx.env;

                // Build the event object using JsObject; the TSFN will pass it to the JS
                // callback as a single argument.
                #[allow(deprecated)]
                let mut event = env.create_object()?;

                let filter_id = BigInt {
                    sign_bit: false,
                    words: ctx.value.filter_id.as_limbs().to_vec(),
                };
                event.set_named_property("filterId", filter_id)?;

                let result: Unknown<'_> = match ctx.value.result {
                    SubscriptionEventData::Logs(logs) => env.to_js_value(&logs)?,
                    SubscriptionEventData::NewHeads(block_to_js_value_fn) => {
                        block_to_js_value_fn(&env)?
                    }
                    SubscriptionEventData::NewPendingTransactions(tx_hash) => {
                        env.to_js_value(&tx_hash)?
                    }
                };

                event.set_named_property("result", result)?;

                Ok(event)
            })?;

        Ok(Self {
            inner: Arc::new(callback),
        })
    }

    pub fn call(&self, event: SubscriptionEvent) {
        // This is blocking because it's important that the subscription events are
        // in-order
        self.inner.call(event, ThreadsafeFunctionCallMode::Blocking);
    }
}

/// Configuration for subscriptions.
#[allow(deprecated)]
pub struct Config<'env> {
    /// Callback to be called when a new event is received.
    pub subscription_callback: Function<'env, JsObject, ()>,
}
