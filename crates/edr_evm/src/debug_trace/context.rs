use edr_eth::{log::ExecutionLog, Address, Bytes, B256, U256};
use revm::context_interface::journaled_state::AccountLoad;
use revm_context_interface::{BlockGetter, CfgGetter, Journal, JournalGetter, TransactionGetter};
use revm_interpreter::{
    interpreter::EthInterpreter, Host, Interpreter, SStoreResult, SelfDestructResult, StateLoad,
};

use super::TracerEip3155;
use crate::instruction::InspectsInstruction;

pub struct Eip3155TracerContext<ContextT> {
    pub(super) inner: ContextT,
    pub(super) tracer: TracerEip3155,
}

impl<ContextT> Eip3155TracerContext<ContextT> {
    /// Creates a new instance.
    pub fn new(tracer: TracerEip3155, inner: ContextT) -> Self {
        Self { inner, tracer }
    }
}

impl<ContextT> BlockGetter for Eip3155TracerContext<ContextT>
where
    ContextT: BlockGetter,
{
    type Block = ContextT::Block;

    fn block(&self) -> &Self::Block {
        self.inner.block()
    }
}

impl<ContextT> CfgGetter for Eip3155TracerContext<ContextT>
where
    ContextT: CfgGetter,
{
    type Cfg = ContextT::Cfg;

    fn cfg(&self) -> &Self::Cfg {
        self.inner.cfg()
    }
}

impl<ContextT> TransactionGetter for Eip3155TracerContext<ContextT>
where
    ContextT: TransactionGetter,
{
    type Transaction = ContextT::Transaction;

    fn tx(&self) -> &Self::Transaction {
        self.inner.tx()
    }
}

impl<ContextT: Host> Host for Eip3155TracerContext<ContextT> {
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

impl<ContextT: JournalGetter> InspectsInstruction for Eip3155TracerContext<ContextT> {
    // TODO: Make this chain-agnostic
    type InterpreterTypes = EthInterpreter;

    fn before_instruction(&self, interpreter: &mut Interpreter<Self::InterpreterTypes>) {
        self.tracer.step(interpreter);
    }

    fn after_instruction(&self, interpreter: &mut Interpreter<Self::InterpreterTypes>) {
        self.tracer.step_end(interpreter, self.inner.journal_ref());
    }
}
