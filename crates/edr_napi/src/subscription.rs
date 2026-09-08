use std::sync::Arc;

// Aliased because this module's own `SubscriptionEvent` shadows the name.
use edr_napi_core::subscription::{
    SubscriptionEvent as CoreSubscriptionEvent, SubscriptionEventData,
};
use edr_primitives::B256;
use edr_provider::{time::TimeSinceEpoch, ProviderSpec, SyncSubscriberCallback};
use napi::{
    bindgen_prelude::{BigInt, Function},
    threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode},
    Env, Unknown,
};
use napi_derive::napi;

use crate::callback::{self, OnOwnerCollected, ProviderCallbacks};

/// Creates a chain-specific [`SyncSubscriberCallback`] for the provided
/// function and chain type.
pub fn subscriber_callback_for_chain_spec<
    ChainSpecT: ProviderSpec<TimerT, Block: 'static, SignedTransaction: 'static>,
    TimerT: Clone + TimeSinceEpoch,
>(
    subscription_callback_fn: Arc<SubscriptionTsfn>,
) -> Box<dyn SyncSubscriberCallback<ChainSpecT::Block, ChainSpecT::SignedTransaction>> {
    Box::new(move |event| {
        let event = CoreSubscriptionEvent::new::<
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
    CoreSubscriptionEvent,
    (),
    SubscriptionEvent<'static>,
    /* ErrorStatus */ napi::Status,
    /* CalleeHandled */ false,
    /* Weak */ { callback::EVENT_LOOP_UNREFERENCED },
    /* MaxQueueSize */ 0,
>;

impl SubscriptionConfig<'_> {
    /// Builds the threadsafe function the provider calls, registering the
    /// consumer's callback in `callbacks`. See [`crate::callback`] for why the
    /// callback is kept out of the threadsafe function.
    pub fn resolve(
        self,
        env: &Env,
        callbacks: &mut ProviderCallbacks,
    ) -> napi::Result<Arc<SubscriptionTsfn>> {
        let subscription_callback = callbacks.unroot_into(
            env,
            self.subscription_callback,
            "subscription",
            // The callback returns nothing, so a late event is dropped.
            OnOwnerCollected::DropCall,
            |trampoline| {
                trampoline
                    .build_threadsafe_function::<CoreSubscriptionEvent>()
                    // Unreferenced from the event loop; see the constant's
                    // docs.
                    .weak::<{ callback::EVENT_LOOP_UNREFERENCED }>()
                    .build_callback(|ctx: ThreadsafeCallContext<CoreSubscriptionEvent>| {
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
                    })
            },
        )?;

        Ok(Arc::new(subscription_callback))
    }
}

#[napi(object)]
pub struct SubscriptionEvent<'env> {
    pub filter_id: BigInt,
    pub result: Unknown<'env>,
}
