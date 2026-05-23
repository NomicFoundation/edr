use std::sync::Arc;

use napi::bindgen_prelude::BigInt;
use napi_derive::napi;

use crate::cast::TryCast as _;

#[napi]
pub struct MockTime {
    inner: Arc<edr_provider::time::MockTime>,
}

impl MockTime {
    pub fn as_inner(&self) -> &Arc<edr_provider::time::MockTime> {
        &self.inner
    }
}

#[napi]
impl MockTime {
    #[doc = "Creates a new instance of `MockTime` with the current time."]
    #[napi(factory, catch_unwind)]
    pub fn now() -> Self {
        Self {
            inner: Arc::new(edr_provider::time::MockTime::now()),
        }
    }

    #[doc = "Adds the specified number of seconds to the current time."]
    #[napi(catch_unwind)]
    pub fn add_seconds(&self, seconds: BigInt) -> napi::Result<()> {
        let seconds = seconds.try_cast()?;

        self.inner.add_seconds(seconds);
        Ok(())
    }
}
