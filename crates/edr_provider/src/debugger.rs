use edr_eth::spec::HaltReasonTrait;
use edr_runtime::trace::TraceCollector;

use crate::{console_log::ConsoleLogCollector, mock::Mocker};

pub struct Debugger<HaltReasonT: HaltReasonTrait> {
    pub console_logger: ConsoleLogCollector,
    pub mocker: Mocker,
    pub trace_collector: TraceCollector<HaltReasonT>,
}

impl<HaltReasonT: HaltReasonTrait> Debugger<HaltReasonT> {
    /// Creates a new instance with the provided mocker.
    /// If verbose is true, full stack and memory will be recorded for each
    /// step.
    pub fn with_mocker(mocker: Mocker, verbose: bool) -> Self {
        Self {
            console_logger: ConsoleLogCollector::default(),
            mocker,
            trace_collector: TraceCollector::new(verbose),
        }
    }
}
