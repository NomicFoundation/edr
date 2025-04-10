use dyn_clone::DynClone;
use edr_coverage::CoverageHitCollector;
use edr_eth::{Bytes, HashSet};

pub trait SyncOnCollectedCoverageCallback: Fn(HashSet<Bytes>) + DynClone + Send + Sync {}

impl<F> SyncOnCollectedCoverageCallback for F where F: Fn(HashSet<Bytes>) + DynClone + Send + Sync {}

dyn_clone::clone_trait_object!(SyncOnCollectedCoverageCallback);

/// A reporter for code coverage that collects hits and reports them to a
/// callback.
pub struct CodeCoverageReporter {
    pub collector: CoverageHitCollector,
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
    pub fn report(self) {
        let hits = self.collector.into_hits();
        (self.callback)(hits);
    }
}
