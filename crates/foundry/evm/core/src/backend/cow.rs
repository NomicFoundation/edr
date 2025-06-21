//! A wrapper around `Backend` that is clone-on-write used for fuzzing.

use std::{borrow::Cow, collections::BTreeMap};

use alloy_genesis::GenesisAccount;
use alloy_primitives::{Address, B256, U256};
use derive_where::derive_where;
use eyre::WrapErr;
use foundry_fork_db::DatabaseError;
use revm::{
    bytecode::Bytecode,
    context::{result::HaltReasonTr, Cfg, JournalInner},
    context_interface::result::ResultAndState,
    database::DatabaseRef,
    primitives::HashMap as Map,
    state::{Account, AccountInfo},
    Database, DatabaseCommit, InspectEvm, JournalEntry,
};

use super::{BackendError, CheatcodeInspectorTr};
use crate::{
    backend::{
        diagnostic::RevertDiagnostic, Backend, CheatcodeBackend, LocalForkId,
        RevertStateSnapshotAction,
    },
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, EvmContext, EvmEnv, EvmEnvWithChainContext,
        HardforkTr, IntoEvmContext as _, TransactionEnvTr, TransactionErrorTrait,
    },
    fork::{CreateFork, ForkId},
};

/// A wrapper around `Backend` that ensures only `revm::DatabaseRef` functions
/// are called.
///
/// Any changes made during its existence that affect the caching layer of the
/// underlying Database will result in a clone of the initial Database.
/// Therefore, this backend type is basically a clone-on-write `Backend`, where
/// cloning is only necessary if cheatcodes will modify the `Backend`
///
/// Entire purpose of this type is for fuzzing. A test function fuzzer will
/// repeatedly execute the function via immutable raw (no state changes) calls.
///
/// **N.B.**: we're assuming cheatcodes that alter the state (like multi fork
/// swapping) are niche. If they executed, it will require a clone of the
/// initial input database. This way we can support these cheatcodes cheaply
/// without adding overhead for tests that don't make use of them. Alternatively
/// each test case would require its own `Backend` clone, which would add
/// significant overhead for large fuzz sets even if the Database is not big
/// after setup.
#[derive_where(Clone, Debug; BlockT, HardforkT, TxT)]
pub struct CowBackend<
    'cow,
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
> {
    /// The underlying `Backend`.
    ///
    /// No calls on the `CowBackend` will ever persistently modify the
    /// `backend`'s state.
    pub backend: Cow<
        'cow,
        Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>,
    >,
    /// Keeps track of whether the backed is already initialized
    is_initialized: bool,
    /// The [`SpecId`] of the current backend.
    spec_id: HardforkT,
}

impl<
        'cow,
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
    >
    CowBackend<
        'cow,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    /// Creates a new `CowBackend` with the given `Backend`.
    pub fn new(
        backend: &'cow Backend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    ) -> Self {
        Self {
            backend: Cow::Borrowed(backend),
            is_initialized: false,
            spec_id: HardforkT::default(),
        }
    }

    /// Executes the configured transaction of the `env` without committing
    /// state changes
    ///
    /// Note: in case there are any cheatcodes executed that modify the
    /// environment, this will update the given `env` with the new values.
    pub fn inspect<'b, InspectorT>(
        &'b mut self,
        env: &mut EvmEnv<BlockT, TxT, HardforkT>,
        inspector: InspectorT,
        chain_context: ChainContextT,
    ) -> eyre::Result<ResultAndState<HaltReasonT>>
    where
        InspectorT: CheatcodeInspectorTr<BlockT, TxT, HardforkT, &'b mut Self, ChainContextT>,
    {
        // this is a new call to inspect with a new env, so even if we've cloned the
        // backend already, we reset the initialized state
        self.is_initialized = false;
        self.spec_id = env.cfg.spec();
        let env_with_chain = EvmEnvWithChainContext::new(env.clone(), chain_context);
        let mut evm = EvmBuilderT::evm_with_inspector(self, env_with_chain, inspector);

        let res = evm
            .inspect_replay()
            .wrap_err("backend: failed while inspecting")?;

        *env = EvmEnv::from(evm.into_evm_context());

        Ok(res)
    }

    /// Returns whether there was a state snapshot failure in the backend.
    ///
    /// This is bubbled up from the underlying Copy-On-Write backend when a
    /// revert occurs.
    pub fn has_state_snapshot_failure(&self) -> bool {
        self.backend.has_state_snapshot_failure()
    }

    /// Returns a mutable instance of the Backend.
    ///
    /// If this is the first time this is called, the backed is cloned and
    /// initialized.
    fn backend_mut(
        &mut self,
        mut env: EvmEnv<BlockT, TxT, HardforkT>,
    ) -> &mut Backend<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    > {
        if !self.is_initialized {
            let backend = self.backend.to_mut();

            env.cfg.spec = self.spec_id;
            backend.initialize(&env);

            self.is_initialized = true;
            return backend;
        }
        self.backend.to_mut()
    }

    /// Returns a mutable instance of the Backend if it is initialized.
    fn initialized_backend_mut(
        &mut self,
    ) -> Option<
        &mut Backend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    > {
        if self.is_initialized {
            return Some(self.backend.to_mut());
        }
        None
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
    >
    CheatcodeBackend<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
    for CowBackend<
        '_,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    fn snapshot_state(
        &mut self,
        journaled_state: &JournalInner<JournalEntry>,
        env: EvmEnv<BlockT, TxT, HardforkT>,
    ) -> U256 {
        self.backend_mut(env.clone())
            .snapshot_state(journaled_state, env)
    }

    fn revert_state<'b>(
        &'b mut self,
        id: U256,
        action: RevertStateSnapshotAction,
        context: &'b mut EvmContext<'b, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> Option<JournalInner<JournalEntry>> {
        self.backend_mut(context.to_owned_env())
            .revert_state(id, action, context)
    }

    fn delete_state_snapshot(&mut self, id: U256) -> bool {
        // delete snapshot requires a previous snapshot to be initialized
        if let Some(backend) = self.initialized_backend_mut() {
            return backend.delete_state_snapshot(id);
        }
        false
    }

    fn delete_state_snapshots(&mut self) {
        if let Some(backend) = self.initialized_backend_mut() {
            backend.delete_state_snapshots();
        }
    }

    fn create_fork(
        &mut self,
        fork: CreateFork<BlockT, TxT, HardforkT>,
    ) -> eyre::Result<LocalForkId> {
        self.backend.to_mut().create_fork(fork)
    }

    fn create_fork_at_transaction(
        &mut self,
        fork: CreateFork<BlockT, TxT, HardforkT>,
        transaction: B256,
        chain_context: &mut ChainContextT,
    ) -> eyre::Result<LocalForkId> {
        self.backend
            .to_mut()
            .create_fork_at_transaction(fork, transaction, chain_context)
    }

    fn select_fork(
        &mut self,
        id: LocalForkId,
        context: &mut EvmContext<'_, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> eyre::Result<()> {
        self.backend_mut(context.to_owned_env())
            .select_fork(id, context)
    }

    fn roll_fork(
        &mut self,
        id: Option<LocalForkId>,
        block_number: u64,
        context: &mut EvmContext<'_, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> eyre::Result<()> {
        self.backend_mut(context.to_owned_env())
            .roll_fork(id, block_number, context)
    }

    fn roll_fork_to_transaction<'a, 'b, 'c>(
        &'a mut self,
        id: Option<LocalForkId>,
        transaction: B256,
        context: &'b mut EvmContext<'c, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> eyre::Result<()>
    where
        'a: 'c,
    {
        self.backend_mut(context.to_owned_env())
            .roll_fork_to_transaction(id, transaction, context)
    }

    fn transact<InspectorT>(
        &mut self,
        id: Option<LocalForkId>,
        transaction: B256,
        inspector: &mut InspectorT,
        env: EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT>,
        journaled_state: &mut JournalInner<JournalEntry>,
    ) -> eyre::Result<()>
    where
        InspectorT: CheatcodeInspectorTr<
            BlockT,
            TxT,
            HardforkT,
            Backend<
                BlockT,
                TxT,
                EvmBuilderT,
                HaltReasonT,
                HardforkT,
                TransactionErrorT,
                ChainContextT,
            >,
            ChainContextT,
        >,
    {
        self.backend_mut(env.clone().into()).transact(
            id,
            transaction,
            inspector,
            env,
            journaled_state,
        )
    }

    fn active_fork_id(&self) -> Option<LocalForkId> {
        self.backend.active_fork_id()
    }

    fn active_fork_url(&self) -> Option<String> {
        self.backend.active_fork_url()
    }

    fn ensure_fork(&self, id: Option<LocalForkId>) -> eyre::Result<LocalForkId> {
        self.backend.ensure_fork(id)
    }

    fn ensure_fork_id(&self, id: LocalForkId) -> eyre::Result<&ForkId> {
        self.backend.ensure_fork_id(id)
    }

    fn diagnose_revert(
        &self,
        callee: Address,
        journaled_state: &JournalInner<JournalEntry>,
    ) -> Option<RevertDiagnostic> {
        self.backend.diagnose_revert(callee, journaled_state)
    }

    fn load_allocs(
        &mut self,
        allocs: &BTreeMap<Address, GenesisAccount>,
        journaled_state: &mut JournalInner<JournalEntry>,
    ) -> Result<(), BackendError> {
        self.backend_mut(EvmEnv::default())
            .load_allocs(allocs, journaled_state)
    }

    fn is_persistent(&self, acc: &Address) -> bool {
        self.backend.is_persistent(acc)
    }

    fn remove_persistent_account(&mut self, account: &Address) -> bool {
        self.backend.to_mut().remove_persistent_account(account)
    }

    fn add_persistent_account(&mut self, account: Address) -> bool {
        self.backend.to_mut().add_persistent_account(account)
    }

    fn allow_cheatcode_access(&mut self, account: Address) -> bool {
        self.backend.to_mut().allow_cheatcode_access(account)
    }

    fn revoke_cheatcode_access(&mut self, account: &Address) -> bool {
        self.backend.to_mut().revoke_cheatcode_access(account)
    }

    fn has_cheatcode_access(&self, account: &Address) -> bool {
        self.backend.has_cheatcode_access(account)
    }

    fn record_cheatcode_purity(&mut self, cheatcode_name: &'static str, is_pure: bool) {
        // Only convert to mutable if we need to update.
        if !is_pure
            && !self
                .backend
                .inner
                .impure_cheatcodes
                .contains(cheatcode_name)
        {
            self.backend
                .to_mut()
                .inner
                .impure_cheatcodes
                .insert(cheatcode_name);
        }
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
    > DatabaseRef
    for CowBackend<
        '_,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    type Error = DatabaseError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        DatabaseRef::basic_ref(self.backend.as_ref(), address)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        DatabaseRef::code_by_hash_ref(self.backend.as_ref(), code_hash)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        DatabaseRef::storage_ref(self.backend.as_ref(), address, index)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        DatabaseRef::block_hash_ref(self.backend.as_ref(), number)
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
    > Database
    for CowBackend<
        '_,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    type Error = DatabaseError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        DatabaseRef::basic_ref(self, address)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        DatabaseRef::code_by_hash_ref(self, code_hash)
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        DatabaseRef::storage_ref(self, address, index)
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        DatabaseRef::block_hash_ref(self, number)
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
    > DatabaseCommit
    for CowBackend<
        '_,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    fn commit(&mut self, changes: Map<Address, Account>) {
        self.backend.to_mut().commit(changes);
    }
}
