pub use revm::primitives::{SpecId, B256};
use revm::{db::EmptyDB, primitives::HandlerCfg, Evm};

pub struct TestOpt<'evm> {
    pub evm: revm::Evm<'evm, (), EmptyDB>,
}

impl<'evm> Default for TestOpt<'evm> {
    fn default() -> Self {
        let evm = Evm::builder()
            .with_empty_db()
            .with_handler_cfg(HandlerCfg::new_with_optimism(SpecId::LATEST, true))
            .build();

        Self { evm }
    }
}
