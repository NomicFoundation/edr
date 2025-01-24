use edr_evm::extension::ExtendedContext;

use super::ConsoleLogCollector;

/// Trait for retrieving a mutable reference to a [`ConsoleLogCollector`]
/// instance.
pub trait ConsoleLogCollectorMutGetter {
    /// Retrieves a mutable reference to a [`ConsoleLogCollector`] instance.
    fn console_log_collector_mut(&mut self) -> &mut ConsoleLogCollector;
}

impl ConsoleLogCollectorMutGetter for ConsoleLogContext<'_> {
    fn console_log_collector_mut(&mut self) -> &mut ConsoleLogCollector {
        self.collector
    }
}

impl<InnerContextT, OuterContextT> ConsoleLogCollectorMutGetter
    for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    OuterContextT: ConsoleLogCollectorMutGetter,
{
    fn console_log_collector_mut(&mut self) -> &mut ConsoleLogCollector {
        self.extension.console_log_collector_mut()
    }
}

/// An EVM context that can be used to collect console logs.
pub struct ConsoleLogContext<'context> {
    collector: &'context mut ConsoleLogCollector,
}

impl<'context> ConsoleLogContext<'context> {
    /// Creates a new instance.
    pub fn new(collector: &'context mut ConsoleLogCollector) -> Self {
        Self { collector }
    }
}
