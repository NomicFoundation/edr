use edr_eth::{log::ExecutionLog, Address, Bytes, B256, U256};
use revm::{context_interface::journaled_state::AccountLoad, JournalEntry};
use revm_context_interface::{
    block::BlockSetter, BlockGetter, CfgGetter, DatabaseGetter, ErrorGetter, Journal,
    JournalGetter, PerformantContextAccess, TransactionGetter,
};
use revm_interpreter::{
    interpreter::EthInterpreter, Host, Interpreter, SStoreResult, SelfDestructResult, StateLoad,
};

use super::TracerEip3155;
use crate::{instruction::InspectsInstruction, spec::ContextForChainSpec};

/// Helper type for a chain-specific [`Eip3155TracerContext`].
pub type Eip3155TracerContextForChainSpec<'tracer, BlockchainT, ChainSpecT, StateT> =
    Eip3155TracerContext<'tracer, ContextForChainSpec<BlockchainT, ChainSpecT, StateT>>;

pub struct Eip3155TracerContext<'tracer, ContextT> {
    pub(super) inner: ContextT,
    pub(super) tracer: &'tracer mut TracerEip3155,
}

impl<'tracer, ContextT> Eip3155TracerContext<'tracer, ContextT> {
    /// Creates a new instance.
    pub fn new(tracer: &'tracer mut TracerEip3155, inner: ContextT) -> Self {
        Self { inner, tracer }
    }
}

impl<'tracer, ContextT> BlockGetter for Eip3155TracerContext<'tracer, ContextT>
where
    ContextT: BlockGetter,
{
    type Block = ContextT::Block;

    fn block(&self) -> &Self::Block {
        self.inner.block()
    }
}

impl<'tracer, ContextT> BlockSetter for Eip3155TracerContext<'tracer, ContextT>
where
    ContextT: BlockSetter,
{
    fn set_block(&mut self, block: <Self as BlockGetter>::Block) {
        self.inner.set_block(block)
    }
}

impl<'tracer, ContextT> CfgGetter for Eip3155TracerContext<'tracer, ContextT>
where
    ContextT: CfgGetter,
{
    type Cfg = ContextT::Cfg;

    fn cfg(&self) -> &Self::Cfg {
        self.inner.cfg()
    }
}

impl<'tracer, ContextT> DatabaseGetter for Eip3155TracerContext<'tracer, ContextT>
where
    ContextT: DatabaseGetter,
{
    type Database = ContextT::Database;

    fn db(&mut self) -> &mut Self::Database {
        self.inner.db()
    }

    fn db_ref(&self) -> &Self::Database {
        self.inner.db_ref()
    }
}

impl<'tracer, ContextT> ErrorGetter for Eip3155TracerContext<'tracer, ContextT>
where
    ContextT: ErrorGetter,
{
    type Error = ContextT::Error;

    fn take_error(&mut self) -> Result<(), Self::Error> {
        self.inner.take_error()
    }
}

impl<'tracer, ContextT: Host> Host for Eip3155TracerContext<'tracer, ContextT> {
    fn load_account_delegated(&mut self, address: Address) -> Option<StateLoad<AccountLoad>> {
        self.inner.load_account_delegated(address)
    }

    fn block_hash(&mut self, number: u64) -> Option<B256> {
        self.inner.block_hash(number)
    }

    fn balance(&mut self, address: Address) -> Option<StateLoad<U256>> {
        self.inner.balance(address)
    }

    fn code(&mut self, address: Address) -> Option<StateLoad<Bytes>> {
        self.inner.code(address)
    }

    fn code_hash(&mut self, address: Address) -> Option<StateLoad<B256>> {
        self.inner.code_hash(address)
    }

    fn sload(&mut self, address: Address, index: U256) -> Option<StateLoad<U256>> {
        self.inner.sload(address, index)
    }

    fn sstore(
        &mut self,
        address: Address,
        index: U256,
        value: U256,
    ) -> Option<StateLoad<SStoreResult>> {
        self.inner.sstore(address, index, value)
    }

    fn tload(&mut self, address: Address, index: U256) -> U256 {
        self.inner.tload(address, index)
    }

    fn tstore(&mut self, address: Address, index: U256, value: U256) {
        self.inner.tstore(address, index, value)
    }

    fn log(&mut self, log: ExecutionLog) {
        self.inner.log(log)
    }

    fn selfdestruct(
        &mut self,
        address: Address,
        target: Address,
    ) -> Option<StateLoad<SelfDestructResult>> {
        self.inner.selfdestruct(address, target)
    }
}

impl<'tracer, ContextT> InspectsInstruction for Eip3155TracerContext<'tracer, ContextT>
where
    ContextT: JournalGetter<Journal: Journal<Entry = JournalEntry>>,
{
    // TODO: Make this chain-agnostic
    type InterpreterTypes = EthInterpreter;

    fn before_instruction(&mut self, interpreter: &mut Interpreter<Self::InterpreterTypes>) {
        self.tracer.step(interpreter);
    }

    fn after_instruction(&mut self, interpreter: &mut Interpreter<Self::InterpreterTypes>) {
        self.tracer.step_end(interpreter, self.inner.journal_ref());
    }
}

impl<'tracer, ContextT> JournalGetter for Eip3155TracerContext<'tracer, ContextT>
where
    ContextT: JournalGetter,
{
    type Journal = ContextT::Journal;

    fn journal(&mut self) -> &mut Self::Journal {
        self.inner.journal()
    }

    fn journal_ref(&self) -> &Self::Journal {
        self.inner.journal_ref()
    }
}

impl<'tracer, ContextT> PerformantContextAccess for Eip3155TracerContext<'tracer, ContextT>
where
    ContextT: PerformantContextAccess,
{
    type Error = ContextT::Error;

    fn load_access_list(&mut self) -> Result<(), Self::Error> {
        self.inner.load_access_list()
    }
}

impl<'tracer, ContextT> TransactionGetter for Eip3155TracerContext<'tracer, ContextT>
where
    ContextT: TransactionGetter,
{
    type Transaction = ContextT::Transaction;

    fn tx(&self) -> &Self::Transaction {
        self.inner.tx()
    }
}
