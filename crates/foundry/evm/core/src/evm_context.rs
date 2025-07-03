use alloy_primitives::{Address, Bytes, TxKind, B256, U256};
use foundry_fork_db::DatabaseError;
use op_revm::{OpEvm, OpTransaction};
use revm::{
    context::{
        either::Either,
        result::{EVMError, HaltReasonTr, InvalidTransaction, ResultAndState},
        transaction::SignedAuthorization,
        BlockEnv, CfgEnv, Evm, JournalInner, LocalContext, TxEnv,
    },
    context_interface::{transaction::AccessList, Block, JournalTr, Transaction},
    handler::{instructions::EthInstructions, EthPrecompiles, PrecompileProvider},
    interpreter::{interpreter::EthInterpreter, InterpreterResult},
    primitives::hardfork::SpecId,
    Database, InspectEvm, Inspector, Journal, JournalEntry,
};

use crate::{
    backend::CheatcodeBackend,
    opts::{BlockEnvOpts, TxEnvOpts},
};

pub trait HardforkTr:
    'static + Copy + std::fmt::Debug + Default + Into<SpecId> + Send + Sync + Unpin
{
}

impl<T> HardforkTr for T where
    T: 'static + Copy + std::fmt::Debug + Default + Into<SpecId> + Send + Sync + Unpin
{
}

// Into and from `BlockEnv` are temporarily needed for compatibility with
// foundry-fork-db
pub trait BlockEnvTr:
    'static
    + Clone
    + std::fmt::Debug
    + Default
    + From<BlockEnvOpts>
    + From<BlockEnv>
    + Into<BlockEnv>
    + Block
    + BlockEnvMut
    + Send
    + Sync
    + Unpin
{
}

impl<T> BlockEnvTr for T where
    T: 'static
        + Clone
        + std::fmt::Debug
        + Default
        + From<BlockEnvOpts>
        + From<BlockEnv>
        + Into<BlockEnv>
        + Block
        + BlockEnvMut
        + Send
        + Sync
        + Unpin
{
}

// Type alias to simplify the context type used in the EVM.
pub type EthInstructionsContext<BlockT, TxT, HardforkT, DatabaseT, ChainContextT> =
    revm::Context<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, Journal<DatabaseT>, ChainContextT>;

pub trait EvmBuilderTrait<
    BlockT: BlockEnvTr,
    ChainContextT,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    TransactionT: TransactionEnvTr,
>
{
    /// Type of the EVM being built.
    type Evm<
        DatabaseT: Database,
        InspectorT: Inspector<
            EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>,
            EthInterpreter,
        >,
    >: InspectEvm<Block = BlockT, Inspector = InspectorT, Tx = TransactionT, Output = Result<
        ResultAndState<HaltReasonT>, EVMError<DatabaseT::Error, TransactionErrorT>
    >> + IntoEvmContext<
        BlockT,
        ChainContextT,
        DatabaseT,
        HardforkT,
        TransactionT
    >;

    /// Type of the precompile provider used in the EVM.
    type PrecompileProvider<DatabaseT: Database>: Default
        + PrecompileProvider<
            EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>,
            Output = InterpreterResult,
        >;

    fn evm_with_inspector<
        DatabaseT: Database,
        InspectorT: Inspector<
            EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>,
            EthInterpreter,
        >,
    >(
        db: DatabaseT,
        env: EvmEnvWithChainContext<BlockT, TransactionT, HardforkT, ChainContextT>,
        inspector: InspectorT,
    ) -> Self::Evm<DatabaseT, InspectorT>;
}

pub struct L1EvmBuilder;

impl
    EvmBuilderTrait<
        BlockEnv,
        (),
        revm::context::result::HaltReason,
        SpecId,
        InvalidTransaction,
        TxEnv,
    > for L1EvmBuilder
{
    type Evm<
        DatabaseT: Database,
        InspectorT: Inspector<EthInstructionsContext<BlockEnv, TxEnv, SpecId, DatabaseT, ()>, EthInterpreter>,
    > = revm::context::Evm<
        EthInstructionsContext<BlockEnv, TxEnv, SpecId, DatabaseT, ()>,
        InspectorT,
        EthInstructions<
            EthInterpreter,
            EthInstructionsContext<BlockEnv, TxEnv, SpecId, DatabaseT, ()>,
        >,
        Self::PrecompileProvider<DatabaseT>,
    >;

    type PrecompileProvider<DatabaseT: Database> = EthPrecompiles;

    fn evm_with_inspector<
        DatabaseT: Database,
        InspectorT: Inspector<EthInstructionsContext<BlockEnv, TxEnv, SpecId, DatabaseT, ()>, EthInterpreter>,
    >(
        db: DatabaseT,
        env: EvmEnvWithChainContext<BlockEnv, TxEnv, SpecId, ()>,
        inspector: InspectorT,
    ) -> Self::Evm<DatabaseT, InspectorT> {
        let mut journaled_state = Journal::<_, JournalEntry>::new(db);
        journaled_state.set_spec_id(env.cfg.spec);

        let context = revm::Context {
            tx: env.tx,
            block: env.block,
            cfg: env.cfg,
            journaled_state,
            chain: env.chain_context,
            local: LocalContext::default(),
            error: Ok(()),
        };

        Evm::new_with_inspector(
            context,
            inspector,
            EthInstructions::default(),
            EthPrecompiles::default(),
        )
    }
}

/// Trait to convert an instance into its inner EVM context type.
pub trait IntoEvmContext<
    BlockT: BlockEnvTr,
    ChainContextT,
    DatabaseT: Database,
    HardforkT: HardforkTr,
    TransactionT: TransactionEnvTr,
>
{
    /// Converts the instance into its inner EVM context type.
    fn into_evm_context(
        self,
    ) -> EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>;
}

impl<
        BlockT: BlockEnvTr,
        ChainContextT,
        DatabaseT: Database,
        HardforkT: HardforkTr,
        InspectorT,
        PrecompileProviderT,
        TransactionT: TransactionEnvTr,
    > IntoEvmContext<BlockT, ChainContextT, DatabaseT, HardforkT, TransactionT>
    for Evm<
        EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>,
        InspectorT,
        EthInstructions<
            EthInterpreter,
            EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>,
        >,
        PrecompileProviderT,
    >
{
    fn into_evm_context(
        self,
    ) -> EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT> {
        self.ctx
    }
}

impl<
        BlockT: BlockEnvTr,
        ChainContextT,
        DatabaseT: Database,
        HardforkT: HardforkTr,
        InspectorT,
        PrecompileProviderT,
        TransactionT: TransactionEnvTr,
    > IntoEvmContext<BlockT, ChainContextT, DatabaseT, HardforkT, TransactionT>
    for OpEvm<
        EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>,
        InspectorT,
        EthInstructions<
            EthInterpreter,
            EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>,
        >,
        PrecompileProviderT,
    >
{
    fn into_evm_context(
        self,
    ) -> EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT> {
        self.0.ctx
    }
}

pub trait TransactionEnvTr:
    'static
    + Clone
    + std::fmt::Debug
    + Default
    + From<TxEnvOpts>
    + Transaction
    + TransactionEnvMut
    + Send
    + Sync
    + Unpin
{
}

impl<T> TransactionEnvTr for T where
    T: 'static
        + Clone
        + std::fmt::Debug
        + Default
        + From<TxEnvOpts>
        + Transaction
        + TransactionEnvMut
        + Send
        + Sync
        + Unpin
{
}

pub trait TransactionErrorTrait:
    'static + From<InvalidTransaction> + std::error::Error + Send + Sync
{
}

impl<TransactionErrorT> TransactionErrorTrait for TransactionErrorT where
    TransactionErrorT: 'static + From<InvalidTransaction> + std::error::Error + Send + Sync
{
}

pub trait ChainContextTr: Clone + std::fmt::Debug + Default {}

impl<T> ChainContextTr for T where T: Clone + std::fmt::Debug + Default {}

pub trait TransactionEnvMut {
    fn set_access_list(&mut self, access_list: AccessList);
    fn set_authorization_list(&mut self, authorization_list: Vec<SignedAuthorization>);
    fn set_blob_versioned_hashes(&mut self, blob_hashes: Vec<B256>);
    fn set_caller(&mut self, caller: Address);
    fn set_chain_id(&mut self, chain_id: Option<u64>);
    fn set_gas_limit(&mut self, gas_limit: u64);
    fn set_gas_price(&mut self, gas_price: u128);
    fn set_gas_priority_fee(&mut self, gas_priority_fee: Option<u128>);
    fn set_max_fee_per_blob_gas(&mut self, max_fee_per_blob_gas: u128);
    fn set_nonce(&mut self, nonce: u64);
    fn set_input(&mut self, input: Bytes);
    fn set_transact_to(&mut self, kind: TxKind);
    fn set_value(&mut self, value: U256);
}

impl TransactionEnvMut for TxEnv {
    fn set_access_list(&mut self, access_list: AccessList) {
        self.access_list = access_list;
    }

    fn set_authorization_list(&mut self, authorization_list: Vec<SignedAuthorization>) {
        self.authorization_list = authorization_list.into_iter().map(Either::Left).collect();
    }

    fn set_blob_versioned_hashes(&mut self, blob_hashes: Vec<B256>) {
        self.blob_hashes = blob_hashes;
    }

    fn set_caller(&mut self, caller: Address) {
        self.caller = caller;
    }

    fn set_chain_id(&mut self, chain_id: Option<u64>) {
        self.chain_id = chain_id;
    }

    fn set_gas_limit(&mut self, gas_limit: u64) {
        self.gas_limit = gas_limit;
    }

    fn set_gas_price(&mut self, gas_price: u128) {
        self.gas_price = gas_price;
    }

    fn set_gas_priority_fee(&mut self, gas_priority_fee: Option<u128>) {
        self.gas_priority_fee = gas_priority_fee;
    }

    fn set_max_fee_per_blob_gas(&mut self, max_fee_per_blob_gas: u128) {
        self.max_fee_per_blob_gas = max_fee_per_blob_gas;
    }

    fn set_nonce(&mut self, nonce: u64) {
        self.nonce = nonce;
    }

    fn set_input(&mut self, input: Bytes) {
        self.data = input;
    }

    fn set_transact_to(&mut self, kind: TxKind) {
        self.kind = kind;
    }

    fn set_value(&mut self, value: U256) {
        self.value = value;
    }
}

impl TransactionEnvMut for OpTransaction<TxEnv> {
    fn set_access_list(&mut self, access_list: AccessList) {
        self.base.access_list = access_list;
    }

    fn set_authorization_list(&mut self, authorization_list: Vec<SignedAuthorization>) {
        self.base.authorization_list = authorization_list.into_iter().map(Either::Left).collect();
    }

    fn set_blob_versioned_hashes(&mut self, blob_hashes: Vec<B256>) {
        self.base.blob_hashes = blob_hashes;
    }

    fn set_caller(&mut self, caller: Address) {
        self.base.caller = caller;
    }

    fn set_chain_id(&mut self, chain_id: Option<u64>) {
        self.base.chain_id = chain_id;
    }

    fn set_gas_limit(&mut self, gas_limit: u64) {
        self.base.gas_limit = gas_limit;
    }

    fn set_gas_price(&mut self, gas_price: u128) {
        self.base.gas_price = gas_price;
    }

    fn set_gas_priority_fee(&mut self, gas_priority_fee: Option<u128>) {
        self.base.gas_priority_fee = gas_priority_fee;
    }

    fn set_max_fee_per_blob_gas(&mut self, max_fee_per_blob_gas: u128) {
        self.base.max_fee_per_blob_gas = max_fee_per_blob_gas;
    }

    fn set_nonce(&mut self, nonce: u64) {
        self.base.nonce = nonce;
    }

    fn set_input(&mut self, input: Bytes) {
        self.base.data = input;
    }

    fn set_transact_to(&mut self, kind: TxKind) {
        self.base.kind = kind;
    }

    fn set_value(&mut self, value: U256) {
        self.base.value = value;
    }
}

pub trait BlockEnvMut {
    fn set_basefee(&mut self, basefee: u64);
    fn set_beneficiary(&mut self, beneficiary: Address);
    fn set_block_number(&mut self, block_number: u64);
    fn set_blob_excess_gas_and_price(&mut self, excess_blob_gas: u64, is_prague: bool);
    fn set_difficulty(&mut self, difficulty: U256);
    fn set_gas_limit(&mut self, gas_limit: u64);
    fn set_prevrandao(&mut self, prevrandao: B256);
    fn set_timestamp(&mut self, timestamp: u64);
}

impl BlockEnvMut for BlockEnv {
    fn set_basefee(&mut self, basefee: u64) {
        self.basefee = basefee;
    }

    fn set_blob_excess_gas_and_price(&mut self, excess_blob_gas: u64, is_prague: bool) {
        self.set_blob_excess_gas_and_price(excess_blob_gas, is_prague);
    }

    fn set_beneficiary(&mut self, coinbase: Address) {
        self.beneficiary = coinbase;
    }

    fn set_difficulty(&mut self, difficulty: U256) {
        self.difficulty = difficulty;
    }

    fn set_prevrandao(&mut self, prevrandao: B256) {
        self.prevrandao = Some(prevrandao);
    }

    fn set_block_number(&mut self, block_number: u64) {
        self.number = block_number;
    }

    fn set_timestamp(&mut self, timestamp: u64) {
        self.timestamp = timestamp;
    }

    fn set_gas_limit(&mut self, gas_limit: u64) {
        self.gas_limit = gas_limit;
    }
}

/// Split the database from EVM execution context so that a mutable method can
/// be called on the database with arguments from the execution context.
pub fn split_context<
    BlockT,
    TxT,
    EvmBuilderT,
    HaltReasonT,
    HardforkT,
    TransactionErrorT,
    DatabaseT,
    ChainContextT,
>(
    context: &mut revm::context::Context<
        BlockT,
        TxT,
        CfgEnv<HardforkT>,
        DatabaseT,
        Journal<DatabaseT>,
        ChainContextT,
    >,
) -> (
    &mut DatabaseT,
    EvmContext<'_, BlockT, TxT, HardforkT, ChainContextT>,
)
where
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT:
        EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
    DatabaseT: CheatcodeBackend<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
{
    let evm_context = EvmContext {
        block: &mut context.block,
        tx: &mut context.tx,
        cfg: &mut context.cfg,
        journaled_state: &mut context.journaled_state.inner,
        chain_context: &mut context.chain,
    };

    (&mut context.journaled_state.database, evm_context)
}

pub struct EvmContext<'a, BlockT, TxT, HardforkT, ChainContextT> {
    pub block: &'a mut BlockT,
    pub tx: &'a mut TxT,
    pub cfg: &'a mut CfgEnv<HardforkT>,
    pub journaled_state: &'a mut JournalInner<JournalEntry>,
    pub chain_context: &'a mut ChainContextT,
}

impl<'a, BlockT, TxT, HardforkT, ChainContextT, DatabaseT>
    From<
        &'a mut revm::context::Context<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
    > for EvmContext<'a, BlockT, TxT, HardforkT, ChainContextT>
where
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    HardforkT: HardforkTr,
    ChainContextT: ChainContextTr,
    DatabaseT: Database<Error = DatabaseError>,
{
    fn from(
        value: &'a mut revm::context::Context<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
    ) -> Self {
        Self {
            block: &mut value.block,
            tx: &mut value.tx,
            cfg: &mut value.cfg,
            journaled_state: &mut value.journaled_state,
            chain_context: &mut value.chain,
        }
    }
}

impl<BlockT, TxT, HardforkT, ChainContextT> EvmContext<'_, BlockT, TxT, HardforkT, ChainContextT>
where
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    HardforkT: HardforkTr,
    ChainContextT: ChainContextTr,
{
    pub fn to_owned_env(&self) -> EvmEnv<BlockT, TxT, HardforkT> {
        EvmEnv {
            block: self.block.clone(),
            tx: self.tx.clone(),
            cfg: self.cfg.clone(),
        }
    }

    pub fn to_owned_env_with_chain_context(
        &self,
    ) -> EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT> {
        EvmEnvWithChainContext {
            block: self.block.clone(),
            tx: self.tx.clone(),
            cfg: self.cfg.clone(),
            chain_context: self.chain_context.clone(),
        }
    }
}

/// EVM execution environment
#[derive(Clone, Debug, Default)]
pub struct EvmEnv<BlockT, TxT, HardforkT> {
    pub block: BlockT,
    pub tx: TxT,
    pub cfg: CfgEnv<HardforkT>,
}

impl<BlockT, TxT, HardforkT, DatabaseT, JournalT, ChainT>
    From<revm::context::Context<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, JournalT, ChainT>>
    for EvmEnv<BlockT, TxT, HardforkT>
where
    DatabaseT: Database,
    JournalT: JournalTr<Database = DatabaseT>,
{
    fn from(
        value: revm::context::Context<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, JournalT, ChainT>,
    ) -> Self {
        Self {
            block: value.block,
            tx: value.tx,
            cfg: value.cfg,
        }
    }
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr>
    EvmEnv<BlockT, TxT, HardforkT>
{
    pub fn new_with_spec_id(spec_id: HardforkT) -> Self {
        let mut env = Self::default();
        env.cfg.spec = spec_id;
        env
    }
}

/// EVM execution environment with chain context.
#[derive(Clone, Debug, Default)]
pub struct EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT> {
    pub block: BlockT,
    pub tx: TxT,
    pub cfg: CfgEnv<HardforkT>,
    pub chain_context: ChainContextT,
}

impl<BlockT, TxT, HardforkT, ChainContextT>
    EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT>
{
    pub fn new(env: EvmEnv<BlockT, TxT, HardforkT>, chain_context: ChainContextT) -> Self {
        Self {
            block: env.block,
            tx: env.tx,
            cfg: env.cfg,
            chain_context,
        }
    }
}

// `EvmEnvWithChainContext` implementation with mainnet types.
impl EvmEnvWithChainContext<BlockEnv, TxEnv, SpecId, ()> {
    pub fn default_mainnet_with_spec_id(spec_id: SpecId) -> Self {
        let mut cfg = CfgEnv::<SpecId>::default();
        cfg.spec = spec_id;

        Self::from_mainnet(cfg, BlockEnv::default(), TxEnv::default())
    }

    pub fn from_mainnet(cfg: CfgEnv<SpecId>, block: BlockEnv, tx: TxEnv) -> Self {
        Self {
            block,
            tx,
            cfg,
            chain_context: (),
        }
    }
}

impl<BlockT, TxT, HardforkT, ChainContextT>
    From<EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT>>
    for EvmEnv<BlockT, TxT, HardforkT>
{
    fn from(value: EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT>) -> Self {
        Self {
            block: value.block,
            tx: value.tx,
            cfg: value.cfg,
        }
    }
}

#[cfg(test)]
mod tests {
    use revm::{database_interface::EmptyDB, inspector::NoOpInspector, ExecuteEvm};

    use super::*;

    #[test]
    fn build_evm() {
        let env = EvmEnvWithChainContext::default_mainnet_with_spec_id(SpecId::default());
        let mut db = EmptyDB::default();

        let mut inspector = NoOpInspector;

        let mut evm = L1EvmBuilder::evm_with_inspector(&mut db, env, &mut inspector);
        let result = evm.transact(revm::context::TxEnv::default()).unwrap();
        assert!(result.result.is_success());
    }
}
