use core::marker::PhantomData;

use edr_eth::{log::ExecutionLog, Address, Bytes, B256, U256};
use revm::JournalEntry;
use revm_context_interface::{
    block::BlockSetter, journaled_state::AccountLoad, BlockGetter, CfgGetter, DatabaseGetter,
    ErrorGetter, Journal, JournalGetter, PerformantContextAccess, TransactionGetter,
};
use revm_interpreter::{Host, Interpreter, SStoreResult, SelfDestructResult, StateLoad};

use crate::instruction::{InspectsInstruction, InspectsInstructionWithJournal};

// /// Type for registering handles, specialised for EDR database component
// types. pub type HandleRegister<'evm, ChainSpecT, BlockchainErrorT,
// DebugDataT, StateT> =     revm::handler::register::HandleRegister<
//         <ChainSpecT as RuntimeSpec>::EvmWiring<
//             WrapDatabaseRef<
//                 DatabaseComponents<
//                     &'evm dyn SyncBlockchain<
//                         ChainSpecT,
//                         BlockchainErrorT,
//                         <StateT as State>::Error,
//                     >,
//                     StateT,
//                 >,
//             >,
//             DebugDataT,
//         >,
//     >;

pub struct ExtendedContext<'context, InnerContextT, OuterContextT> {
    pub inner: InnerContextT,
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

impl<'context, InnerContextT, OuterContextT> BlockGetter
    for ExtendedContext<'context, InnerContextT, OuterContextT>
where
    InnerContextT: BlockGetter,
{
    type Block = InnerContextT::Block;

    fn block(&self) -> &Self::Block {
        self.inner.block()
    }
}

impl<'context, InnerContextT, OuterContextT> BlockSetter
    for ExtendedContext<'context, InnerContextT, OuterContextT>
where
    InnerContextT: BlockSetter,
{
    fn set_block(&mut self, block: <Self as BlockGetter>::Block) {
        self.inner.set_block(block)
    }
}

impl<'context, InnerContextT, OuterContextT> CfgGetter
    for ExtendedContext<'context, InnerContextT, OuterContextT>
where
    InnerContextT: CfgGetter,
{
    type Cfg = InnerContextT::Cfg;

    fn cfg(&self) -> &Self::Cfg {
        self.inner.cfg()
    }
}

impl<'context, InnerContextT, OuterContextT> DatabaseGetter
    for ExtendedContext<'context, InnerContextT, OuterContextT>
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

impl<'context, InnerContextT, OuterContextT> ErrorGetter
    for ExtendedContext<'context, InnerContextT, OuterContextT>
where
    InnerContextT: ErrorGetter,
{
    type Error = InnerContextT::Error;

    fn take_error(&mut self) -> Result<(), Self::Error> {
        self.inner.take_error()
    }
}

impl<'context, InnerContextT: Host, OuterContextT> Host
    for ExtendedContext<'context, InnerContextT, OuterContextT>
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

impl<'context, InnerContextT, OuterContextT> InspectsInstruction
    for ExtendedContext<'context, InnerContextT, OuterContextT>
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

impl<'context, InnerContextT, OuterContextT> JournalGetter
    for ExtendedContext<'context, InnerContextT, OuterContextT>
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

impl<'context, InnerContextT, OuterContextT> PerformantContextAccess
    for ExtendedContext<'context, InnerContextT, OuterContextT>
where
    InnerContextT: PerformantContextAccess,
{
    type Error = InnerContextT::Error;

    fn load_access_list(&mut self) -> Result<(), Self::Error> {
        self.inner.load_access_list()
    }
}

impl<'context, InnerContextT, OuterContextT> TransactionGetter
    for ExtendedContext<'context, InnerContextT, OuterContextT>
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
pub struct ContextExtension<ExtensionT, FrameT> {
    extension: ExtensionT,
    phantom: PhantomData<FrameT>,
    // /// The handler
    // pub handler: HandlerT,
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
    pub fn extend_context<'context, ContextT>(
        &'context mut self,
        inner: ContextT,
    ) -> ExtendedContext<'context, ContextT, ExtensionT> {
        ExtendedContext {
            inner,
            extension: &mut self.extension,
        }
    }
}

// pub type NoopContextConstructor<BlockchainT, ChainSpecT, StateT> =
//     fn(
//         ContextForChainSpec<BlockchainT, ChainSpecT, StateT>,
//     ) -> ContextForChainSpec<BlockchainT, ChainSpecT, StateT>;
