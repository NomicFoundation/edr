use edr_evm::extension::ExtendedContext;

use super::Mocker;

/// Trait for retrieving a mutable reference to a [`Mocker`] instance.
pub trait MockerMutGetter {
    /// Retrieves a mutable reference to a [`Mocker`] instance.
    fn mocker_mut(&mut self) -> &mut Mocker;
}

impl MockerMutGetter for MockingContext<'_> {
    fn mocker_mut(&mut self) -> &mut Mocker {
        self.context
    }
}

impl<InnerContextT, OuterContextT> MockerMutGetter
    for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    OuterContextT: MockerMutGetter,
{
    fn mocker_mut(&mut self) -> &mut Mocker {
        self.extension.mocker_mut()
    }
}

/// An EVM context that can be used to mock calls.
pub struct MockingContext<'context> {
    context: &'context mut Mocker,
}

impl<'context> MockingContext<'context> {
    /// Creates a new instance.
    pub fn new(context: &'context mut Mocker) -> Self {
        Self { context }
    }
}
