mod context;
mod frame;

use core::fmt::Debug;
use std::sync::Arc;

use dyn_clone::DynClone;
use edr_eth::{Address, Bytes};

pub use self::{
    context::{MockerMutGetter, MockingContext},
    frame::MockingFrame,
};

/// The result of executing a call override.
#[derive(Debug)]
pub struct CallOverrideResult {
    pub output: Bytes,
    pub should_revert: bool,
}

pub trait SyncCallOverride:
    Fn(Address, Bytes) -> Option<CallOverrideResult> + DynClone + Send + Sync
{
}

impl<F> SyncCallOverride for F where
    F: Fn(Address, Bytes) -> Option<CallOverrideResult> + DynClone + Send + Sync
{
}

dyn_clone::clone_trait_object!(SyncCallOverride);

pub struct Mocker {
    call_override: Option<Arc<dyn SyncCallOverride>>,
}

impl Mocker {
    /// Constructs a new instance with the provided call override.
    pub fn new(call_override: Option<Arc<dyn SyncCallOverride>>) -> Self {
        Self { call_override }
    }

    fn override_call(&self, contract: Address, input: Bytes) -> Option<CallOverrideResult> {
        self.call_override.as_ref().and_then(|f| f(contract, input))
    }
}
