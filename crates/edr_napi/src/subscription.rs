use std::sync::Arc;

use edr_napi_core::subscription::SubscriptionEventData;
use edr_primitives::B256;
use edr_provider::{time::TimeSinceEpoch, ProviderSpec, SyncSubscriberCallback};
use napi::{
    bindgen_prelude::{BigInt, Function},
    threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode},
    Unknown,
};
use napi_derive::napi;

/// Creates a chain-specific [`SyncSubscriberCallback`] for the provided
/// function and chain type.
pub fn subscriber_callback_for_chain_spec<
    ChainSpecT: ProviderSpec<TimerT, Block: 'static, SignedTransaction: 'static>,
    TimerT: Clone + TimeSinceEpoch,
>(
    subscription_callback_fn: Arc<SubscriptionTsfn>,
) -> Box<dyn SyncSubscriberCallback<ChainSpecT::Block, ChainSpecT::SignedTransaction>> {
    Box::new(move |event| {
        let event = edr_napi_core::subscription::SubscriptionEvent::new::<
            ChainSpecT::Block,
            ChainSpecT::RpcBlock<B256>,
            ChainSpecT::SignedTransaction,
        >(event);

        // This is blocking because it's important that the subscription events are
        // in-order
        subscription_callback_fn.call(event, ThreadsafeFunctionCallMode::Blocking);
    })
}

/// Configuration for subscriptions.
#[napi(object)]
pub struct SubscriptionConfig<'env> {
    /// Callback to be called when a new event is received.
    pub subscription_callback: Function<'env, SubscriptionEvent<'static>, ()>,
}

pub type SubscriptionTsfn = ThreadsafeFunction<
    edr_napi_core::subscription::SubscriptionEvent,
    (),
    SubscriptionEvent<'static>,
    /* ErrorStatus */ napi::Status,
    /* CalleeHandled */ false,
    /* Weak */ true,
    /* MaxQueueSize */ 0,
>;

impl SubscriptionConfig<'_> {
    pub fn resolve(self) -> napi::Result<Arc<SubscriptionTsfn>> {
        let subscription_event_callback_fn = self
            .subscription_callback
            .build_threadsafe_function::<edr_napi_core::subscription::SubscriptionEvent>()
            // Maintain a weak reference to the function to avoid blocking
            // the event loop from exiting.
            .weak::<true>()
            .build_callback(
                |ctx: ThreadsafeCallContext<edr_napi_core::subscription::SubscriptionEvent>| {
                    let env = ctx.env;

                    let filter_id = BigInt {
                        sign_bit: false,
                        words: ctx.value.filter_id.as_limbs().to_vec(),
                    };

                    let result: Unknown<'static> = match ctx.value.result {
                        SubscriptionEventData::Logs(logs) => env.to_js_value(&logs)?,
                        SubscriptionEventData::NewHeads(block_to_js_value_fn) => {
                            block_to_js_value_fn(&env)?
                        }
                        SubscriptionEventData::NewPendingTransactions(tx_hash) => {
                            env.to_js_value(&tx_hash)?
                        }
                    };

                    Ok(SubscriptionEvent { filter_id, result })
                },
            )?;

        Ok(Arc::new(subscription_event_callback_fn))
    }
}

#[napi(object)]
pub struct SubscriptionEvent<'env> {
    pub filter_id: BigInt,
    pub result: Unknown<'env>,
}
