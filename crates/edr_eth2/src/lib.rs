use revm::{db::EmptyDB, Evm};

pub struct TestEth<'evm> {
    pub evm: revm::Evm<'evm, (), EmptyDB>,
}

impl<'evm> Default for TestEth<'evm> {
    fn default() -> Self {
        let evm = Evm::builder().with_empty_db().build();
        Self { evm }
    }
}
