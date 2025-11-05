use core::fmt::Debug;

use dyn_clone::DynClone;

use crate::{State, StateCommit, StateDebug, StateError};

/// Super-trait for dynamic trait objects that implement all state
/// functionalities.
pub trait DynState:
    State<Error = StateError>
    + StateCommit
    + StateDebug<Error = StateError>
    + Debug
    + DynClone
    + Send
    + Sync
{
}

impl Clone for Box<dyn DynState> {
    fn clone(&self) -> Self {
        dyn_clone::clone_box(&**self)
    }
}

impl<StateT> DynState for StateT where
    StateT: State<Error = StateError>
        + StateCommit
        + StateDebug<Error = StateError>
        + Debug
        + DynClone
        + Send
        + Sync
{
}
