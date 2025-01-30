use core::marker::PhantomData;

use edr_eth::{log::ExecutionLog, Address, Bytes, HashMap, B256, U256};
use revm::JournalEntry;
use revm_context_interface::{
    block::BlockSetter, journaled_state::AccountLoad, BlockGetter, CfgGetter, DatabaseGetter,
    ErrorGetter, Journal, JournalGetter, PerformantContextAccess, TransactionGetter,
};
use revm_interpreter::{Host, Interpreter, SStoreResult, SelfDestructResult, StateLoad};

use crate::{
    instruction::{InspectsInstruction, InspectsInstructionWithJournal},
    precompile::{CustomPrecompilesGetter, PrecompileFn},
};

/// An extended context consisting of an inner context for execution and an
/// extension for runtime observability.
pub struct ExtendedContext<'context, InnerContextT, OuterContextT> {
    /// The inner context for execution.
    pub inner: InnerContextT,
    /// The extension for runtime observability.
    pub extension: &'context mut OuterContextT,
}

impl<'context, InnerContextT, OuterContextT>
    ExtendedContext<'context, InnerContextT, OuterContextT>
{
    /// Creates a new instance.
    pub fn new(inner: InnerContextT, extension: &'context mut OuterContextT) -> Self {
        Self { inner, extension }
    }
}

impl<InnerContextT, OuterContextT> BlockGetter for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    InnerContextT: BlockGetter,
{
    type Block = InnerContextT::Block;

    fn block(&self) -> &Self::Block {
        self.inner.block()
    }
}

impl<InnerContextT, OuterContextT> BlockSetter for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    InnerContextT: BlockSetter,
{
    fn set_block(&mut self, block: <Self as BlockGetter>::Block) {
        self.inner.set_block(block);
    }
}

impl<InnerContextT, OuterContextT> CfgGetter for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    InnerContextT: CfgGetter,
{
    type Cfg = InnerContextT::Cfg;

    fn cfg(&self) -> &Self::Cfg {
        self.inner.cfg()
    }
}

impl<InnerContextT, OuterContextT> CustomPrecompilesGetter
    for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    OuterContextT: CustomPrecompilesGetter,
{
    fn custom_precompiles(&self) -> HashMap<Address, PrecompileFn> {
        self.extension.custom_precompiles()
    }
}

impl<InnerContextT, OuterContextT> DatabaseGetter
    for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    InnerContextT: DatabaseGetter,
{
    type Database = InnerContextT::Database;

    fn db(&mut self) -> &mut Self::Database {
        self.inner.db()
    }

    fn db_ref(&self) -> &Self::Database {
        self.inner.db_ref()
    }
}

impl<InnerContextT, OuterContextT> ErrorGetter for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    InnerContextT: ErrorGetter,
{
    type Error = InnerContextT::Error;

    fn take_error(&mut self) -> Result<(), Self::Error> {
        self.inner.take_error()
    }
}

impl<InnerContextT: Host, OuterContextT> Host
    for ExtendedContext<'_, InnerContextT, OuterContextT>
{
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
        self.inner.tstore(address, index, value);
    }

    fn log(&mut self, log: ExecutionLog) {
        self.inner.log(log);
    }

    fn selfdestruct(
        &mut self,
        address: Address,
        target: Address,
    ) -> Option<StateLoad<SelfDestructResult>> {
        self.inner.selfdestruct(address, target)
    }
}

impl<InnerContextT, OuterContextT> InspectsInstruction
    for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    InnerContextT: JournalGetter<Journal: Journal<Entry = JournalEntry>>,
    OuterContextT: InspectsInstructionWithJournal<Journal = InnerContextT::Journal>,
{
    // TODO: Make this chain-agnostic
    type InterpreterTypes = OuterContextT::InterpreterTypes;

    fn before_instruction(&mut self, interpreter: &Interpreter<Self::InterpreterTypes>) {
        let journal = self.inner.journal_ref();
        self.extension
            .before_instruction_with_journal(interpreter, journal);
    }

    fn after_instruction(&mut self, interpreter: &Interpreter<Self::InterpreterTypes>) {
        let journal = self.inner.journal_ref();
        self.extension
            .after_instruction_with_journal(interpreter, journal);
    }
}

impl<InnerContextT, OuterContextT> JournalGetter
    for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    InnerContextT: JournalGetter,
{
    type Journal = InnerContextT::Journal;

    fn journal(&mut self) -> &mut Self::Journal {
        self.inner.journal()
    }

    fn journal_ref(&self) -> &Self::Journal {
        self.inner.journal_ref()
    }
}

impl<InnerContextT, OuterContextT> PerformantContextAccess
    for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    InnerContextT: PerformantContextAccess,
{
    type Error = InnerContextT::Error;

    fn load_access_list(&mut self) -> Result<(), Self::Error> {
        self.inner.load_access_list()
    }
}

impl<InnerContextT, OuterContextT> TransactionGetter
    for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    InnerContextT: TransactionGetter,
{
    type Transaction = InnerContextT::Transaction;

    fn tx(&self) -> &Self::Transaction {
        self.inner.tx()
    }
}

/// Type for encapsulating contextual data and handler registration in an
/// `EvmBuilder`.
///
/// # Usage
///
/// It only seems possible to use `ExtensionT` types that exclusively contain
/// (mutable) references to data. Tested in Rust v1.83.
pub struct ContextExtension<ExtensionT, FrameT> {
    extension: ExtensionT,
    phantom: PhantomData<FrameT>,
}

impl<ExtensionT, FrameT> ContextExtension<ExtensionT, FrameT> {
    /// Creates a new instance.
    pub fn new(extension: ExtensionT) -> Self {
        Self {
            extension,
            phantom: PhantomData,
        }
    }

    /// Extends the provided context.
    pub fn extend_context<ContextT>(
        &mut self,
        inner: ContextT,
    ) -> ExtendedContext<'_, ContextT, ExtensionT> {
        ExtendedContext {
            inner,
            extension: &mut self.extension,
        }
    }
}
