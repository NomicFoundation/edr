use derive_more::Debug;
use dyn_clone::DynClone;
use edr_primitives::{Bytes, HashSet};

use crate::CoverageHitCollector;

pub trait SyncOnCollectedCoverageCallback:
    Fn(HashSet<Bytes>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + DynClone + Send + Sync
{
}

impl<F> SyncOnCollectedCoverageCallback for F where
    F: Fn(HashSet<Bytes>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
        + DynClone
        + Send
        + Sync
{
}

dyn_clone::clone_trait_object!(SyncOnCollectedCoverageCallback);

/// A reporter for code coverage that collects hits and reports them to a
/// callback.
#[derive(Clone, Debug)]
pub struct CodeCoverageReporter {
    pub collector: CoverageHitCollector,
    #[debug(skip)]
    callback: Box<dyn SyncOnCollectedCoverageCallback>,
}

impl CodeCoverageReporter {
    /// Creates a new instance with the provided callback.
    pub fn new(callback: Box<dyn SyncOnCollectedCoverageCallback>) -> Self {
        Self {
            collector: CoverageHitCollector::default(),
            callback,
        }
    }

    /// Reports the collected coverage hits to the callback.
    pub fn collect_and_report(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let hits = self.collector.into_hits();
        (self.callback)(hits)
    }

    /// Flushes the collected coverage hits to the callback, replacing the
    /// current hits with an empty set.
    pub fn flush_and_report(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let hits = self.collector.take();
        (self.callback)(hits)
    }
}
