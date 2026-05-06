use napi::bindgen_prelude::{BigInt, Function};
#[allow(deprecated)]
use napi::JsObject;
use napi_derive::napi;

/// Configuration for subscriptions.
#[allow(deprecated)]
#[napi(object)]
pub struct SubscriptionConfig<'env> {
    /// Callback to be called when a new event is received.
    #[napi(ts_type = "(event: SubscriptionEvent) => void")]
    pub subscription_callback: Function<'env, JsObject, ()>,
}

impl<'env> From<edr_napi_core::subscription::Config<'env>> for SubscriptionConfig<'env> {
    fn from(config: edr_napi_core::subscription::Config<'env>) -> Self {
        Self {
            subscription_callback: config.subscription_callback,
        }
    }
}

impl<'env> From<SubscriptionConfig<'env>> for edr_napi_core::subscription::Config<'env> {
    fn from(config: SubscriptionConfig<'env>) -> Self {
        Self {
            subscription_callback: config.subscription_callback,
        }
    }
}

#[napi(object)]
pub struct SubscriptionEvent {
    pub filter_id: BigInt,
    pub result: serde_json::Value,
}
