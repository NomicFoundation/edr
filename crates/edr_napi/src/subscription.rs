// `JsObject` (used as the TSFN args type for the subscription event callback)
// is `#[deprecated]` in napi-rs, but the typed `Object<'_>` does not
// implement `ToNapiValue` by value, so it cannot be used as the
// `JsValuesTupleIntoVec` arg of a `ThreadsafeFunction`. The deprecation
// allow has to live at module scope because the `#[napi(object)]` macro
// expands into impl blocks outside the struct where a struct-level
// `#[allow]` would not reach.
#![allow(deprecated)]

use napi::{
    bindgen_prelude::{BigInt, Function},
    JsObject,
};
use napi_derive::napi;

/// Configuration for subscriptions.
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
