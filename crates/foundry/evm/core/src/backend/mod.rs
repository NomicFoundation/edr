//! Foundry's main executor backend abstraction and implementation.

use crate::{
    backend::predeploy::insert_predeploys,
    constants::{CALLER, CHEATCODE_ADDRESS, DEFAULT_CREATE2_DEPLOYER, TEST_CONTRACT_ADDRESS},
    evm_context::{
        EvmBuilderTrait, IntoEvmContext, TransactionErrorTrait,
        BlockEnvTr, ChainContextTr, EvmContext, EvmEnv, EvmEnvWithChainContext, HardforkTr,
        TransactionEnvTr, TransactionEnvMut
    },
    fork::{CreateFork, ForkId, MultiFork},
    state_snapshot::StateSnapshots,
    utils::{configure_tx_env, get_blob_base_fee_update_fraction_by_spec_id},
};
use alloy_genesis::GenesisAccount;
use alloy_network::{AnyRpcBlock, AnyTxEnvelope, TransactionResponse};
use alloy_primitives::{Address, B256, TxKind, U256, address, keccak256, uint};
use alloy_rpc_types::{BlockNumberOrTag, Transaction as RpcTransaction};
use eyre::Context;
pub use foundry_fork_db::{BlockchainDb, SharedBackend, cache::BlockchainDbMeta};
use revm::{
    Database, DatabaseCommit, InspectEvm, Inspector, Journal, JournalEntry,
    bytecode::Bytecode,
    context::{CfgEnv, JournalInner, result::HaltReasonTr},
    context_interface::{Cfg, result::ResultAndState},
    database::{CacheDB, DatabaseRef},
    inspector::NoOpInspector,
    precompile::{PrecompileSpecId, Precompiles},
    primitives::{HashMap as Map, KECCAK_EMPTY, Log, hardfork::SpecId},
    state::{Account, AccountInfo, EvmState, EvmStorageSlot},
};
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Debug,
    marker::PhantomData,
    time::Instant,
};

use derive_where::derive_where;
use serde::{Deserialize, Serialize};

mod diagnostic;
pub use diagnostic::RevertDiagnostic;

mod error;
pub use error::{BackendError, BackendResult, DatabaseError, DatabaseResult};

mod cow;
pub use cow::CowBackend;

mod in_memory_db;
pub use in_memory_db::{EmptyDBWrapper, FoundryEvmInMemoryDB, MemDb};

mod predeploy;
pub use predeploy::Predeploy;

mod snapshot;
pub use snapshot::{BackendStateSnapshot, RevertStateSnapshotAction, StateSnapshot};

// A `revm::Database` that is used in forking mode
type ForkDB = CacheDB<SharedBackend>;

/// Represents a numeric `ForkId` valid only for the existence of the `Backend`.
///
/// The difference between `ForkId` and `LocalForkId` is that `ForkId` tracks pairs of `endpoint +
/// block` which can be reused by multiple tests, whereas the `LocalForkId` is unique within a test
pub type LocalForkId = U256;

/// Represents the index of a fork in the created forks vector
/// This is used for fast lookup
type ForkLookupIndex = usize;

/// All accounts that will have persistent storage across fork swaps.
const DEFAULT_PERSISTENT_ACCOUNTS: [Address; 3] =
    [CHEATCODE_ADDRESS, DEFAULT_CREATE2_DEPLOYER, CALLER];

/// Arbitrum L1 sender address of the first transaction in every block.
/// `0x00000000000000000000000000000000000a4b05`
const ARBITRUM_SENDER: Address = address!("00000000000000000000000000000000000a4b05");

/// The system address, the sender of the first transaction in every block:
/// `0xdeaddeaddeaddeaddeaddeaddeaddeaddead0001`
///
/// See also <https://github.com/ethereum-optimism/optimism/blob/65ec61dde94ffa93342728d324fecf474d228e1f/specs/deposits.md#l1-attributes-deposited-transaction>
const OPTIMISM_SYSTEM_ADDRESS: Address = address!("deaddeaddeaddeaddeaddeaddeaddeaddead0001");

/// Transaction identifier of System transaction types
const SYSTEM_TRANSACTION_TYPE: u8 = 126;

/// `bytes32("failed")`, as a storage slot key into [`CHEATCODE_ADDRESS`].
///
/// Used by all `forge-std` test contracts and newer `DSTest` test contracts as a global marker for
/// a failed test.
pub const GLOBAL_FAIL_SLOT: U256 =
    uint!(0x6661696c65640000000000000000000000000000000000000000000000000000_U256);

pub type JournaledState = JournalInner<JournalEntry>;

/// Helper trait to reduce typing generics for `revm::Inspector`
pub trait CheatcodeInspectorTr<BlockT, TxT, HardforkT, DatabaseT, ChainContextT>:
    Inspector<
    revm::context::Context<
        BlockT,
        TxT,
        CfgEnv<HardforkT>,
        DatabaseT,
        Journal<DatabaseT>,
        ChainContextT,
    >,
>
where
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    HardforkT: HardforkTr,
    DatabaseT: Database,
    ChainContextT: ChainContextTr,
{
}

impl<T, BlockT, TxT, HardforkT, DatabaseT, ChainContextT>
    CheatcodeInspectorTr<BlockT, TxT, HardforkT, DatabaseT, ChainContextT> for T
where
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    HardforkT: HardforkTr,
    DatabaseT: Database,
    ChainContextT: ChainContextTr,
    T: Inspector<
        revm::context::Context<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
    >,
{
}

/// An extension trait that allows us to easily extend the `revm::Inspector` capabilities
#[auto_impl::auto_impl(&mut)]
pub trait CheatcodeBackend<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
>: Database<Error = DatabaseError>
{
    /// Creates a new state snapshot at the current point of execution.
    ///
    /// A state snapshot is associated with a new unique id that's created for the snapshot.
    /// State snapshots can be reverted: [`DatabaseExt::revert_state`], however, depending on the
    /// [`RevertStateSnapshotAction`], it will keep the snapshot alive or delete it.
    fn snapshot_state(
        &mut self,
        journaled_state: &JournalInner<JournalEntry>,
        env: EvmEnv<BlockT, TxT, HardforkT>,
    ) -> U256;

    /// Reverts the snapshot if it exists
    ///
    /// Returns `true` if the snapshot was successfully reverted, `false` if no snapshot for that id
    /// exists.
    ///
    /// **N.B.** While this reverts the state of the evm to the snapshot, it keeps new logs made
    /// since the snapshots was created. This way we can show logs that were emitted between
    /// snapshot and its revert.
    /// This will also revert any changes in the `Env` and replace it with the captured `Env` of
    /// `Self::snapshot_state`.
    ///
    /// Depending on [`RevertStateSnapshotAction`] it will keep the snapshot alive or delete it.
    fn revert_state<'a>(
        &'a mut self,
        id: U256,
        action: RevertStateSnapshotAction,
        context: &'a mut EvmContext<'a, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> Option<JournaledState>;

    /// Deletes the state snapshot with the given `id`
    ///
    /// Returns `true` if the snapshot was successfully deleted, `false` if no snapshot for that id
    /// exists.
    fn delete_state_snapshot(&mut self, id: U256) -> bool;

    /// Deletes all state snapshots.
    fn delete_state_snapshots(&mut self);

    /// Creates and also selects a new fork
    ///
    /// This is basically `create_fork` + `select_fork`
    fn create_select_fork<'a>(
        &'a mut self,
        fork: CreateFork<BlockT, TxT, HardforkT>,
        context: &'a mut EvmContext<'a, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> eyre::Result<LocalForkId> {
        let id = self.create_fork(fork)?;
        self.select_fork(id, context)?;
        Ok(id)
    }

    /// Creates and also selects a new fork
    ///
    /// This is basically `create_fork` + `select_fork`
    fn create_select_fork_at_transaction<'a>(
        &'a mut self,
        fork: CreateFork<BlockT, TxT, HardforkT>,
        transaction: B256,
        context: &'a mut EvmContext<'a, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> eyre::Result<LocalForkId> {
        let id = self.create_fork_at_transaction(fork, transaction, context.chain_context)?;
        self.select_fork(id, context)?;
        Ok(id)
    }

    /// Creates a new fork but does _not_ select it
    fn create_fork(
        &mut self,
        fork: CreateFork<BlockT, TxT, HardforkT>,
    ) -> eyre::Result<LocalForkId>;

    /// Creates a new fork but does _not_ select it
    fn create_fork_at_transaction(
        &mut self,
        fork: CreateFork<BlockT, TxT, HardforkT>,
        transaction: B256,
        chain_context: &mut ChainContextT,
    ) -> eyre::Result<LocalForkId>;

    /// Selects the fork's state
    ///
    /// This will also modify the current `EvmEnv<BlockT, TxT, HardforkT>`.
    ///
    /// **Note**: this does not change the local state, but swaps the remote state
    ///
    /// # Errors
    ///
    /// Returns an error if no fork with the given `id` exists
    fn select_fork(
        &mut self,
        id: LocalForkId,
        context: &mut EvmContext<'_, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> eyre::Result<()>;

    /// Updates the fork to given block number.
    ///
    /// This will essentially create a new fork at the given block height.
    ///
    /// # Errors
    ///
    /// Returns an error if not matching fork was found.
    fn roll_fork(
        &mut self,
        id: Option<LocalForkId>,
        block_number: u64,
        context: &mut EvmContext<'_, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> eyre::Result<()>;

    /// Updates the fork to given transaction hash
    ///
    /// This will essentially create a new fork at the block this transaction was mined and replays
    /// all transactions up until the given transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if not matching fork was found.
    fn roll_fork_to_transaction<'a, 'b, 'c>(
        &'a mut self,
        id: Option<LocalForkId>,
        transaction: B256,
        context: &'b mut EvmContext<'c, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> eyre::Result<()>
    where
        'a: 'c;

    /// Fetches the given transaction for the fork and executes it, committing the state in the DB
    fn transact(
        &mut self,
        id: Option<LocalForkId>,
        transaction: B256,
        inspector: &mut dyn CheatcodeInspectorTr<
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
        env: EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT>,
        journaled_state: &mut JournalInner<JournalEntry>,
    ) -> eyre::Result<()>
    where
        Self: Sized;

    /// Returns the `ForkId` that's currently used in the database, if fork mode is on
    fn active_fork_id(&self) -> Option<LocalForkId>;

    /// Returns the Fork url that's currently used in the database, if fork mode is on
    fn active_fork_url(&self) -> Option<String>;

    /// Whether the database is currently in forked mode.
    fn is_forked_mode(&self) -> bool {
        self.active_fork_id().is_some()
    }

    /// Ensures that an appropriate fork exists
    ///
    /// If `id` contains a requested `Fork` this will ensure it exists.
    /// Otherwise, this returns the currently active fork.
    ///
    /// # Errors
    ///
    /// Returns an error if the given `id` does not match any forks
    ///
    /// Returns an error if no fork exists
    fn ensure_fork(&self, id: Option<LocalForkId>) -> eyre::Result<LocalForkId>;

    /// Ensures that a corresponding `ForkId` exists for the given local `id`
    fn ensure_fork_id(&self, id: LocalForkId) -> eyre::Result<&ForkId>;

    /// Handling multiple accounts/new contracts in a multifork environment can be challenging since
    /// every fork has its own standalone storage section. So this can be a common error to run
    /// into:
    ///
    /// ```solidity
    /// function testCanDeploy() public {
    ///    vm.selectFork(mainnetFork);
    ///    // contract created while on `mainnetFork`
    ///    DummyContract dummy = new DummyContract();
    ///    // this will succeed
    ///    dummy.hello();
    ///
    ///    vm.selectFork(optimismFork);
    ///
    ///    vm.expectRevert();
    ///    // this will revert since `dummy` contract only exists on `mainnetFork`
    ///    dummy.hello();
    /// }
    /// ```
    ///
    /// If this happens (`dummy.hello()`), or more general, a call on an address that's not a
    /// contract, revm will revert without useful context. This call will check in this context if
    /// `address(dummy)` belongs to an existing contract and if not will check all other forks if
    /// the contract is deployed there.
    ///
    /// Returns a more useful error message if that's the case
    fn diagnose_revert(
        &self,
        callee: Address,
        journaled_state: &JournaledState,
    ) -> Option<RevertDiagnostic>;

    /// Loads the account allocs from the given `allocs` map into the passed [`JournaledState`].
    ///
    /// Returns [Ok] if all accounts were successfully inserted into the journal, [Err] otherwise.
    fn load_allocs(
        &mut self,
        allocs: &BTreeMap<Address, GenesisAccount>,
        journaled_state: &mut JournaledState,
    ) -> Result<(), BackendError>;

    /// Copies bytecode, storage, nonce and balance from the given genesis account to the target
    /// address.
    ///
    /// Returns [Ok] if data was successfully inserted into the journal, [Err] otherwise.
    fn clone_account(
        &mut self,
        source: &GenesisAccount,
        target: &Address,
        journaled_state: &mut JournaledState,
    ) -> Result<(), BackendError>;

    /// Returns true if the given account is currently marked as persistent.
    fn is_persistent(&self, acc: &Address) -> bool;

    /// Revokes persistent status from the given account.
    fn remove_persistent_account(&mut self, account: &Address) -> bool;

    /// Marks the given account as persistent.
    fn add_persistent_account(&mut self, account: Address) -> bool;

    /// Removes persistent status from all given accounts.
    fn remove_persistent_accounts(&mut self, accounts: impl IntoIterator<Item = Address>)
    where
        Self: Sized,
    {
        for acc in accounts {
            self.remove_persistent_account(&acc);
        }
    }

    /// Extends the persistent accounts with the accounts the iterator yields.
    fn extend_persistent_accounts(&mut self, accounts: impl IntoIterator<Item = Address>)
    where
        Self: Sized,
    {
        for acc in accounts {
            self.add_persistent_account(acc);
        }
    }

    /// Grants cheatcode access for the given `account`
    ///
    /// Returns true if the `account` already has access
    fn allow_cheatcode_access(&mut self, account: Address) -> bool;

    /// Revokes cheatcode access for the given account
    ///
    /// Returns true if the `account` was previously allowed cheatcode access
    fn revoke_cheatcode_access(&mut self, account: &Address) -> bool;

    /// Returns `true` if the given account is allowed to execute cheatcodes
    fn has_cheatcode_access(&self, account: &Address) -> bool;

    /// Ensures that `account` is allowed to execute cheatcodes
    ///
    /// Returns an error if [`Self::has_cheatcode_access`] returns `false`
    fn ensure_cheatcode_access(&self, account: &Address) -> Result<(), BackendError> {
        if !self.has_cheatcode_access(account) {
            return Err(BackendError::NoCheats(*account));
        }
        Ok(())
    }

    /// Same as [`Self::ensure_cheatcode_access()`] but only enforces it if the backend is currently
    /// in forking mode
    fn ensure_cheatcode_access_forking_mode(&self, account: &Address) -> Result<(), BackendError> {
        if self.is_forked_mode() {
            return self.ensure_cheatcode_access(account);
        }
        Ok(())
    }

    // Can't take generic cheatcode as argument as this trait needs to be
    // object-safe.
    fn record_cheatcode_purity(&mut self, cheatcode_name: &'static str, is_pure: bool);

    /// Set the blockhash for a given block number.
    ///
    /// # Arguments
    ///
    /// * `number` - The block number to set the blockhash for
    /// * `hash` - The blockhash to set
    ///
    /// # Note
    ///
    /// This function mimics the EVM limits of the `blockhash` operation:
    /// - It sets the blockhash for blocks where `block.number - 256 <= number < block.number`
    /// - Setting a blockhash for the current block (number == block.number) has no effect
    /// - Setting a blockhash for future blocks (number > block.number) has no effect
    /// - Setting a blockhash for blocks older than `block.number - 256` has no effect
    fn set_blockhash(&mut self, block_number: U256, block_hash: B256);
}

struct _ObjectSafe<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
>(
    dyn CheatcodeBackend<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        Error = DatabaseError,
    >,
);

/// Provides the underlying `revm::Database` implementation.
///
/// A `Backend` can be initialised in two forms:
///
/// # 1. Empty in-memory Database
/// This is the default variant: an empty `revm::Database`
///
/// # 2. Forked Database
/// A `revm::Database` that forks off a remote client
///
///
/// In addition to that we support forking manually on the fly.
/// Additional forks can be created. Each unique fork is identified by its unique `ForkId`. We treat
/// forks as unique if they have the same `(endpoint, block number)` pair.
///
/// When it comes to testing, it's intended that each contract will use its own `Backend`
/// (`Backend::clone`). This way each contract uses its own encapsulated evm state. For in-memory
/// testing, the database is just an owned `revm::InMemoryDB`.
///
/// Each `Fork`, identified by a unique id, uses completely separate storage, write operations are
/// performed only in the fork's own database, `ForkDB`.
///
/// A `ForkDB` consists of 2 halves:
///   - everything fetched from the remote is readonly
///   - all local changes (instructed by the contract) are written to the backend's `db` and don't
///     alter the state of the remote client.
///
/// # Fork swapping
///
/// Multiple "forks" can be created `Backend::create_fork()`, however only 1 can be used by the
/// `db`. However, their state can be hot-swapped by swapping the read half of `db` from one fork to
/// another.
/// When swapping forks (`Backend::select_fork()`) we also update the current `EvmEnv<BlockT, TxT, HardforkT>` of the `EVM`
/// accordingly, so that all `block.*` config values match
///
/// When another for is selected [`CheatcodeBackend::select_fork()`] the entire storage, including
/// `JournaledState` is swapped, but the storage of the caller's and the test contract account is
/// _always_ cloned. This way a fork has entirely separate storage but data can still be shared
/// across fork boundaries via stack and contract variables.
///
/// # Snapshotting
///
/// A snapshot of the current overall state can be taken at any point in time. A snapshot is
/// identified by a unique id that's returned when a snapshot is created. A snapshot can only be
/// reverted _once_. After a successful revert, the same snapshot id cannot be used again. Reverting
/// a snapshot replaces the current active state with the snapshot state, the snapshot is deleted
/// afterwards, as well as any snapshots taken after the reverted snapshot, (e.g.: reverting to id
/// 0x1 will delete snapshots with ids 0x1, 0x2, etc.)
///
/// **Note:** State snapshots work across fork-swaps, e.g. if fork `A` is currently active, then a
/// snapshot is created before fork `B` is selected, then fork `A` will be the active fork again
/// after reverting the snapshot.
#[derive_where(Clone, Debug; BlockT, HardforkT, TxT)]
pub struct Backend<
    BlockT,
    TxT,
    EvmBuilderT,
    HaltReasonT,
    HardforkT,
    TransactionErrorT,
    ChainContextT,
> {
    /// The access point for managing forks
    forks: MultiFork<BlockT, TxT, HardforkT>,
    // The default in memory db
    mem_db: FoundryEvmInMemoryDB,
    /// The `journaled_state` to use to initialize new forks with
    ///
    /// The way [`JournaledState`] works is, that it holds the "hot" accounts loaded from the
    /// underlying `Database` that feeds the Account and State data to the `journaled_state` so it
    /// can apply changes to the state while the EVM executes.
    ///
    /// In a way the `JournaledState` is something like a cache that
    /// 1. check if account is already loaded (hot)
    /// 2. if not load from the `Database` (this will then retrieve the account via RPC in forking
    ///    mode)
    ///
    /// To properly initialize we store the `JournaledState` before the first
    /// fork is selected ([`CheatcodeBackend::select_fork`]).
    ///
    /// This will be an empty `JournaledState`, which will be populated with persistent accounts,
    /// See [`Self::update_fork_db()`].
    fork_init_journaled_state: JournaledState,
    /// The currently active fork database
    ///
    /// If this is set, then the Backend is currently in forking mode
    active_fork_ids: Option<(LocalForkId, ForkLookupIndex)>,
    /// holds additional Backend data
    inner: BackendInner<BlockT, TxT, HardforkT>,

    #[allow(clippy::type_complexity)]
    _phantom: PhantomData<fn() -> (ChainContextT, EvmBuilderT, HaltReasonT, TransactionErrorT)>,
}

// === impl Backend ===

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
    > Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>
{
    /// Creates a new Backend with a spawned multi fork thread.
    pub fn spawn(
        fork: Option<CreateFork<BlockT, TxT, HardforkT>>,
        local_predeploys: impl IntoIterator<Item = Predeploy>,
    ) -> eyre::Result<Self> {
        Self::new(MultiFork::spawn(), fork, local_predeploys)
    }

    /// Creates a new instance of `Backend`
    ///
    /// If `fork` is `Some` this will use a `fork` database, otherwise with an in-memory
    /// database.
    pub fn new(
        forks: MultiFork<BlockT, TxT, HardforkT>,
        fork: Option<CreateFork<BlockT, TxT, HardforkT>>,
        local_predeploys: impl IntoIterator<Item = Predeploy>,
    ) -> eyre::Result<Self> {
        trace!(target: "backend", forking_mode=?fork.is_some(), "creating executor backend");
        // Note: this will take of registering the `fork`
        let inner = BackendInner {
            persistent_accounts: HashSet::from(DEFAULT_PERSISTENT_ACCOUNTS),
            ..Default::default()
        };

        let mut db = CacheDB::new(EmptyDBWrapper::default());
        insert_predeploys(&mut db, local_predeploys);

        let mut backend = Self {
            forks,
            mem_db: db,
            fork_init_journaled_state: inner.new_journaled_state(),
            active_fork_ids: None,
            inner,
            _phantom: PhantomData,
        };

        if let Some(fork) = fork {
            let fork_block_number = fork.evm_opts.fork_block_number;
            let (fork_id, fork, _) =
                backend.forks.create_fork(fork)?;
            let fork_db = ForkDB::new(fork);
            let fork_ids = backend.inner.insert_new_fork(
                fork_id.clone(),
                fork_db,
                backend.inner.new_journaled_state(),
                fork_block_number,
            );
            backend.inner.launched_with_fork = Some(LaunchedWithFork {
                fork_id,
                local_fork_id: fork_ids.0,
                fork_look_up_index: fork_ids.1,
                fork_block_number,
            });
            backend.active_fork_ids = Some(fork_ids);
        }

        trace!(target: "backend", forking_mode=? backend.active_fork_ids.is_some(), "created executor backend");

        Ok(backend)
    }

    /// Creates a new instance of `Backend` with fork added to the fork database and sets the fork
    /// as active
    pub(crate) fn new_with_fork(
        id: &ForkId,
        fork: Fork,
        journaled_state: JournalInner<JournalEntry>,
    ) -> eyre::Result<Self> {
        // No predeploys in fork mode.
        let mut backend = Self::spawn(None, Vec::default())?;
        let fork_block_number = fork.fork_block_number;
        let fork_ids =
            backend.inner.insert_new_fork(id.clone(), fork.db, journaled_state, fork_block_number);
        backend.inner.launched_with_fork = Some(LaunchedWithFork {
            fork_id: id.clone(),
            local_fork_id: fork_ids.0,
            fork_look_up_index: fork_ids.1,
            fork_block_number,
        });
        backend.active_fork_ids = Some(fork_ids);
        Ok(backend)
    }

    /// Creates a new instance with a `BackendDatabase::InMemory` cache layer for the `CacheDB`
    pub fn clone_empty(&self) -> Self {
        Self {
            forks: self.forks.clone(),
            mem_db: CacheDB::new(EmptyDBWrapper::default()),
            fork_init_journaled_state: self.inner.new_journaled_state(),
            active_fork_ids: None,
            inner: BackendInner::default(),
            _phantom: PhantomData,
        }
    }

    pub fn insert_account_info(&mut self, address: Address, account: AccountInfo) {
        if let Some(db) = self.active_fork_db_mut() {
            db.insert_account_info(address, account);
        } else {
            self.mem_db.insert_account_info(address, account);
        }
    }

    /// Inserts a value on an account's storage without overriding account info
    pub fn insert_account_storage(
        &mut self,
        address: Address,
        slot: U256,
        value: U256,
    ) -> Result<(), DatabaseError> {
        if let Some(db) = self.active_fork_db_mut() {
            db.insert_account_storage(address, slot, value)
        } else {
            self.mem_db.insert_account_storage(address, slot, value)
        }
    }

    /// Completely replace an account's storage without overriding account info.
    ///
    /// When forking, this causes the backend to assume a `0` value for all
    /// unset storage slots instead of trying to fetch it.
    pub fn replace_account_storage(
        &mut self,
        address: Address,
        storage: Map<U256, U256>,
    ) -> Result<(), DatabaseError> {
        if let Some(db) = self.active_fork_db_mut() {
            db.replace_account_storage(address, storage)
        } else {
            self.mem_db.replace_account_storage(address, storage)
        }
    }

    /// Returns all snapshots created in this backend
    pub fn state_snapshots(
        &self,
    ) -> &StateSnapshots<BackendStateSnapshot<BackendDatabaseSnapshot, BlockT, TxT, HardforkT>>
    {
        &self.inner.state_snapshots
    }

    /// Sets the address of the `DSTest` contract that is being executed
    ///
    /// This will also mark the caller as persistent and remove the persistent status from the
    /// previous test contract address
    ///
    /// This will also grant cheatcode access to the test account
    pub fn set_test_contract(&mut self, acc: Address) -> &mut Self {
        trace!(?acc, "setting test account");
        self.add_persistent_account(acc);
        self.allow_cheatcode_access(acc);
        self.inner.test_contract_address = Some(acc);
        self
    }

    /// Sets the caller address
    pub fn set_caller(&mut self, acc: Address) -> &mut Self {
        trace!(?acc, "setting caller account");
        self.inner.caller = Some(acc);
        self.allow_cheatcode_access(acc);
        self
    }

    /// Sets the current spec id
    pub fn set_spec_id(&mut self, spec_id: HardforkT) -> &mut Self {
        trace!(?spec_id, "setting spec ID");
        self.inner.spec_id = spec_id;
        self
    }

    /// Returns the address of the set `DSTest` contract
    pub fn test_contract_address(&self) -> Option<Address> {
        self.inner.test_contract_address
    }

    /// Returns the set caller address
    pub fn caller_address(&self) -> Option<Address> {
        self.inner.caller
    }

    /// Failures occurred in state snapshots are tracked when the state snapshot is reverted.
    ///
    /// If an error occurs in a restored state snapshot, the test is considered failed.
    ///
    /// This returns whether there was a reverted state snapshot that recorded an error.
    pub fn has_state_snapshot_failure(&self) -> bool {
        self.inner.has_state_snapshot_failure
    }

    /// Sets the state snapshot failure flag.
    pub fn set_state_snapshot_failure(&mut self, has_state_snapshot_failure: bool) {
        self.inner.has_state_snapshot_failure = has_state_snapshot_failure;
    }

    /// Checks if the test contract associated with this backend failed, See
    /// [`Self::is_failed_test_contract`]
    pub fn is_failed(&self) -> bool {
        self.has_state_snapshot_failure()
            || self
                .test_contract_address()
                .map(|addr| self.is_failed_test_contract(addr))
                .unwrap_or_default()
    }

    /// Checks if the given test function failed
    ///
    /// `DSTest` will not revert inside its `assertEq`-like functions which
    /// allows to test multiple assertions in 1 test function while also
    /// preserving logs. Instead, it stores whether an `assert` failed in a
    /// boolean variable that we can read
    pub fn is_failed_test_contract(&self, address: Address) -> bool {
        /*
         contract DSTest {
            bool public IS_TEST = true;
            // slot 0 offset 1 => second byte of slot0
            bool private _failed;
         }
        */
        let value = self.storage_ref(address, U256::ZERO).unwrap_or_default();
        value.as_le_bytes().get(1).copied().unwrap_or(0) != 0
    }

    /// Checks if the given test function failed by looking at the present value
    /// of the test contract's `JournaledState`
    ///
    /// See [`Self::is_failed_test_contract()]`
    ///
    /// Note: we assume the test contract is either `forge-std/Test` or `DSTest`
    pub fn is_failed_test_contract_state(
        &self,
        address: Address,
        current_state: &JournalInner<JournalEntry>,
    ) -> bool {
        if let Some(account) = current_state.state.get(&address) {
            let value = account
                .storage
                .get(&revm::primitives::U256::ZERO)
                .cloned()
                .unwrap_or_default()
                .present_value();
            return value.as_le_bytes().get(1).copied().unwrap_or(0) != 0;
        }

        false
    }

    /// In addition to the `_failed` variable, `DSTest::fail()` stores a failure
    /// in "failed"
    /// See <https://github.com/dapphub/ds-test/blob/9310e879db8ba3ea6d5c6489a579118fd264a3f5/src/test.sol#L66-L72>
    pub fn is_global_failure(&self, current_state: &JournalInner<JournalEntry>) -> bool {
        if let Some(account) = current_state.state.get(&CHEATCODE_ADDRESS) {
            let slot: U256 = GLOBAL_FAIL_SLOT;
            let value = account.storage.get(&slot).cloned().unwrap_or_default().present_value();
            return value == revm::primitives::U256::from(1);
        }

        false
    }

    /// Whether the backend instance only executed pure cheatcodes.
    /// Returns true if no cheatcodes were executed.
    pub fn are_executed_cheatcodes_pure(&self) -> bool {
        self.inner.impure_cheatcodes.is_empty()
    }

    /// The impure cheatcode signatures that were executed.
    pub fn impure_cheatcodes(&self) -> HashSet<Cow<'static, str>> {
        self.inner.impure_cheatcodes.clone()
    }

    /// Check that the global fork this backend was launched is using the
    /// "latest" block tag.
    pub fn is_global_fork_latest(&self) -> bool {
        self.inner.launched_with_fork.as_ref().is_some_and(|lf| lf.fork_block_number.is_none())
    }

    /// Whether when re-executing the calls the same results are guaranteed.
    pub fn safe_to_re_execute(&self) -> bool {
        !self.is_global_fork_latest() && self.are_executed_cheatcodes_pure()
    }

    /// If re-executing the counter example is not guaranteed to yield the same
    /// results, the `Some` result contains the reason why.
    pub fn indeterminism_reasons(&self) -> Option<IndeterminismReasons> {
        let global_fork_latest = self.is_global_fork_latest();
        if global_fork_latest || !self.are_executed_cheatcodes_pure() {
            Some(IndeterminismReasons {
                global_fork_latest,
                impure_cheatcodes: self.impure_cheatcodes(),
            })
        } else {
            None
        }
    }

    /// When creating or switching forks, we update the `AccountInfo` of the contract
    pub(crate) fn update_fork_db(
        &self,
        active_journaled_state: &mut JournaledState,
        target_fork: &mut Fork,
    ) {
        debug_assert!(
            self.inner.test_contract_address.is_some(),
            "Test contract address must be set"
        );

        self.update_fork_db_contracts(
            self.inner.persistent_accounts.iter().copied(),
            active_journaled_state,
            target_fork,
        );
    }

    /// Merges the state of all `accounts` from the currently active db into the given `fork`
    pub(crate) fn update_fork_db_contracts(
        &self,
        accounts: impl IntoIterator<Item = Address>,
        active_journaled_state: &mut JournaledState,
        target_fork: &mut Fork,
    ) {
        if let Some((_, fork_idx)) = self.active_fork_ids.as_ref() {
            let active = self.inner.get_fork(*fork_idx);
            merge_account_data(accounts, &active.db, active_journaled_state, target_fork);
        } else {
            merge_account_data(accounts, &self.mem_db, active_journaled_state, target_fork);
        }
    }

    /// Returns the memory db used if not in forking mode
    pub fn mem_db(&self) -> &FoundryEvmInMemoryDB {
        &self.mem_db
    }

    /// Returns true if the `id` is currently active
    pub fn is_active_fork(&self, id: LocalForkId) -> bool {
        self.active_fork_ids.map(|(i, _)| i == id).unwrap_or_default()
    }

    /// Returns `true` if the `Backend` is currently in forking mode
    pub fn is_in_forking_mode(&self) -> bool {
        self.active_fork().is_some()
    }

    /// Returns the currently active `Fork`, if any
    pub fn active_fork(&self) -> Option<&Fork> {
        self.active_fork_ids.map(|(_, idx)| self.inner.get_fork(idx))
    }

    /// Returns the currently active `Fork`, if any
    pub fn active_fork_mut(&mut self) -> Option<&mut Fork> {
        self.active_fork_ids.map(|(_, idx)| self.inner.get_fork_mut(idx))
    }

    /// Returns the currently active `ForkDB`, if any
    pub fn active_fork_db(&self) -> Option<&ForkDB> {
        self.active_fork().map(|f| &f.db)
    }

    /// Returns the currently active `ForkDB`, if any
    pub fn active_fork_db_mut(&mut self) -> Option<&mut ForkDB> {
        self.active_fork_mut().map(|f| &mut f.db)
    }

    /// Returns the current database implementation as a `&dyn` value.
    #[inline(always)]
    pub fn db(&self) -> &dyn Database<Error = DatabaseError> {
        match self.active_fork_db() {
            Some(fork_db) => fork_db,
            None => &self.mem_db,
        }
    }

    /// Returns the current database implementation as a `&mut dyn` value.
    #[inline(always)]
    pub fn db_mut(&mut self) -> &mut dyn Database<Error = DatabaseError> {
        match self.active_fork_ids.map(|(_, idx)| &mut self.inner.get_fork_mut(idx).db) {
            Some(fork_db) => fork_db,
            None => &mut self.mem_db,
        }
    }

    /// Creates a snapshot of the currently active database
    pub(crate) fn create_db_snapshot(&self) -> BackendDatabaseSnapshot {
        if let Some((id, idx)) = self.active_fork_ids {
            let fork = self.inner.get_fork(idx).clone();
            let fork_id = self.inner.ensure_fork_id(id).cloned().expect("Exists; qed");
            BackendDatabaseSnapshot::Forked(id, fork_id, idx, Box::new(fork))
        } else {
            BackendDatabaseSnapshot::InMemory(self.mem_db.clone())
        }
    }

    /// Since each `Fork` tracks logs separately, we need to merge them to get _all_ of them
    pub fn merged_logs(&self, mut logs: Vec<Log>) -> Vec<Log> {
        if let Some((_, active)) = self.active_fork_ids {
            let mut all_logs = Vec::with_capacity(logs.len());

            self.inner
                .forks
                .iter()
                .enumerate()
                .filter_map(|(idx, f)| f.as_ref().map(|f| (idx, f)))
                .for_each(|(idx, f)| {
                    if idx == active {
                        all_logs.append(&mut logs);
                    } else {
                        all_logs.extend(f.journaled_state.logs.clone());
                    }
                });
            return all_logs;
        }

        logs
    }

    /// Initializes settings we need to keep track of.
    ///
    /// We need to track these mainly to prevent issues when switching between different evms
    pub(crate) fn initialize(&mut self, env: &EvmEnv<BlockT, TxT, HardforkT>) {
        self.set_caller(env.tx.caller());
        self.set_spec_id(env.cfg.spec());

        let test_contract = match env.tx.kind() {
            TxKind::Call(to) => to,
            TxKind::Create => {
                let nonce = self
                    .basic_ref(env.tx.caller())
                    .map(|b| b.unwrap_or_default().nonce)
                    .unwrap_or_default();
                env.tx.caller().create(nonce)
            }
        };
        self.set_test_contract(test_contract);
    }

    /// Returns the `EvmEnv` with the current `spec_id` set.
    fn env_with_handler_cfg(
        &self,
        mut env: EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT>,
    ) -> EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT> {
        env.cfg.spec = self.inner.spec_id;
        env
    }

    /// Executes the configured test call of the `env` without committing state changes.
    ///
    /// Note: in case there are any cheatcodes executed that modify the environment, this will
    /// update the given `env` with the new values.
    pub fn inspect<'a, InspectorT>(
        &'a mut self,
        env: &mut EvmEnv<BlockT, TxT, HardforkT>,
        chain_context: ChainContextT,
        inspector: InspectorT,
    ) -> eyre::Result<ResultAndState<HaltReasonT>>
    where
        InspectorT: CheatcodeInspectorTr<BlockT, TxT, HardforkT, &'a mut Self, ChainContextT>,
    {
        self.initialize(env);
        let mut env_with_chain = EvmEnvWithChainContext::new(env.clone(), chain_context);
        let tx = std::mem::take(&mut env_with_chain.tx);
        let mut evm = EvmBuilderT::evm_with_inspector(self, env_with_chain, inspector);

        let res = evm.inspect_tx(tx).wrap_err("EVM error")?;

        *env = EvmEnv::from(evm.into_evm_context());

        Ok(res)
    }

    /// Returns true if the address is a precompile
    pub fn is_existing_precompile(&self, addr: &Address) -> bool {
        self.inner.precompiles().contains(addr)
    }

    /// Sets the initial journaled state to use when initializing forks
    #[inline]
    fn set_init_journaled_state(&mut self, journaled_state: JournaledState) {
        trace!("recording fork init journaled_state");
        self.fork_init_journaled_state = journaled_state;
    }

    /// Cleans up already loaded accounts that would be initialized without the correct data from
    /// the fork.
    ///
    /// It can happen that an account is loaded before the first fork is selected, like
    /// `getNonce(addr)`, which will load an empty account by default.
    ///
    /// This account data then would not match the account data of a fork if it exists.
    /// So when the first fork is initialized we replace these accounts with the actual account as
    /// it exists on the fork.
    fn prepare_init_journal_state(&mut self) -> Result<(), BackendError> {
        let loaded_accounts = self
            .fork_init_journaled_state
            .state
            .iter()
            .filter(|(addr, _)| !self.is_existing_precompile(addr) && !self.is_persistent(addr))
            .map(|(addr, _)| addr)
            .copied()
            .collect::<Vec<_>>();

        for fork in self.inner.forks_iter_mut() {
            let mut journaled_state = self.fork_init_journaled_state.clone();
            for loaded_account in loaded_accounts.iter().copied() {
                trace!(?loaded_account, "replacing account on init");
                let init_account =
                    journaled_state.state.get_mut(&loaded_account).expect("exists; qed");

                // here's an edge case where we need to check if this account has been created, in
                // which case we don't need to replace it with the account from the fork because the
                // created account takes precedence: for example contract creation in setups
                if init_account.is_created() {
                    trace!(?loaded_account, "skipping created account");
                    continue;
                }

                // otherwise we need to replace the account's info with the one from the fork's
                // database
                let fork_account = Database::basic(&mut fork.db, loaded_account)?
                    .ok_or(BackendError::MissingAccount(loaded_account))?;
                init_account.info = fork_account;
            }
            fork.journaled_state = journaled_state;
        }
        Ok(())
    }

    /// Returns the block numbers required for replaying a transaction
    fn get_block_number_and_block_for_transaction(
        &self,
        id: LocalForkId,
        transaction: B256,
    ) -> eyre::Result<(u64, AnyRpcBlock)> {
        let fork = self.inner.get_fork_by_id(id)?;
        let tx = fork.db.db.get_transaction(transaction)?;

        // get the block number we need to fork
        if let Some(tx_block) = tx.block_number {
            let block = fork.db.db.get_full_block(tx_block)?;

            // we need to subtract 1 here because we want the state before the transaction
            // was mined
            let fork_block = tx_block - 1;
            Ok((fork_block, block))
        } else {
            let block = fork.db.db.get_full_block(BlockNumberOrTag::Latest)?;

            let number = block.header.number;

            Ok((number, block))
        }
    }

    /// Replays all the transactions at the forks current block that were mined before the `tx`
    ///
    /// Returns the _unmined_ transaction that corresponds to the given `tx_hash`
    pub fn replay_until(
        &mut self,
        id: LocalForkId,
        tx_hash: B256,
        env: EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT>,
        journaled_state: &mut JournalInner<JournalEntry>,
    ) -> eyre::Result<Option<RpcTransaction<AnyTxEnvelope>>> {
        trace!(?id, ?tx_hash, "replay until transaction");

        let persistent_accounts = self.inner.persistent_accounts.clone();
        let fork_id = self.ensure_fork_id(id)?.clone();

        let env = self.env_with_handler_cfg(env);
        let fork = self.inner.get_fork_by_id_mut(id)?;
        let full_block = fork.db.db.get_full_block(env.block.number().saturating_to::<u64>())?;

        for tx in full_block.inner.transactions.txns() {
            // System transactions such as on L2s don't contain any pricing info so we skip them
            // otherwise this would cause reverts
            if is_known_system_sender(tx.from())
                || tx.transaction_type() == Some(SYSTEM_TRANSACTION_TYPE)
            {
                trace!(tx=?tx.tx_hash(), "skipping system transaction");
                continue;
            }

            if tx.tx_hash() == tx_hash {
                // found the target transaction
                return Ok(Some(tx.inner.clone()));
            }
            trace!(tx=?tx.tx_hash(), "committing transaction");

            commit_transaction::<_, _, EvmBuilderT, _, _, _, _, _>(
                &tx.inner,
                env.clone(),
                journaled_state,
                fork,
                &fork_id,
                &persistent_accounts,
                NoOpInspector,
            )?;
        }

        Ok(None)
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
    for Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>
{
    fn snapshot_state(
        &mut self,
        journaled_state: &JournalInner<JournalEntry>,
        env: EvmEnv<BlockT, TxT, HardforkT>,
    ) -> U256 {
        trace!("create snapshot");
        let id = self.inner.state_snapshots.insert(BackendStateSnapshot::new(
            self.create_db_snapshot(),
            journaled_state.clone(),
            env,
        ));
        trace!(target: "backend", "Created new snapshot {}", id);
        id
    }

    fn revert_state(
        &mut self,
        id: U256,
        action: RevertStateSnapshotAction,
        context: &mut EvmContext<'_, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> Option<JournaledState> {
        trace!(?id, "revert snapshot");
        if let Some(mut snapshot) = self.inner.state_snapshots.remove_at(id) {
            // Re-insert snapshot to persist it
            if action.is_keep() {
                self.inner.state_snapshots.insert_at(snapshot.clone(), id);
            }
            // need to check whether there's a global failure which means an error occurred
            // either during the snapshot or even before
            if self.is_global_failure(context.journaled_state) {
                self.set_state_snapshot_failure(true);
            }

            // merge additional logs
            snapshot.merge(context.journaled_state);
            let BackendStateSnapshot { db, mut journaled_state, env } = snapshot;
            match db {
                BackendDatabaseSnapshot::InMemory(mem_db) => {
                    self.mem_db = mem_db;
                }
                BackendDatabaseSnapshot::Forked(id, fork_id, idx, mut fork) => {
                    // there might be the case where the snapshot was created during `setUp` with
                    // another caller, so we need to ensure the caller account is present in the
                    // journaled state and database
                    let caller = context.tx.caller();
                    journaled_state.state.entry(caller).or_insert_with(|| {
                        let caller_account = context
                            .journaled_state
                            .state
                            .get(&caller)
                            .map(|acc| acc.info.clone())
                            .unwrap_or_default();

                        if !fork.db.cache.accounts.contains_key(&caller) {
                            // update the caller account which is required by the evm
                            fork.db.insert_account_info(caller, caller_account.clone());
                        }
                        caller_account.into()
                    });
                    self.inner.revert_state_snapshot(id, fork_id, idx, *fork);
                    self.active_fork_ids = Some((id, idx));
                }
            }

            update_current_env_with_fork_env(context, env);
            trace!(target: "backend", "Reverted snapshot {}", id);

            Some(journaled_state)
        } else {
            warn!(target: "backend", "No snapshot to revert for {}", id);
            None
        }
    }

    fn delete_state_snapshot(&mut self, id: U256) -> bool {
        self.inner.state_snapshots.remove_at(id).is_some()
    }

    fn delete_state_snapshots(&mut self) {
        self.inner.state_snapshots.clear();
    }

    fn create_fork(
        &mut self,
        create_fork: CreateFork<BlockT, TxT, HardforkT>,
    ) -> eyre::Result<LocalForkId> {
        trace!("create fork");
        let fork_block_number = create_fork.evm_opts.fork_block_number;
        let (fork_id, fork, _) = self.forks.create_fork(create_fork)?;

        let fork_db = ForkDB::new(fork);
        let (id, _) = self.inner.insert_new_fork(
            fork_id,
            fork_db,
            self.fork_init_journaled_state.clone(),
            fork_block_number,
        );
        Ok(id)
    }

    fn create_fork_at_transaction(
        &mut self,
        fork: CreateFork<BlockT, TxT, HardforkT>,
        transaction: B256,
        chain_context: &mut ChainContextT,
    ) -> eyre::Result<LocalForkId> {
        trace!(?transaction, "create fork at transaction");
        let id = self.create_fork(fork)?;
        let fork_id = self.ensure_fork_id(id).cloned()?;
        let mut env = self
            .forks
            .get_env(fork_id)?
            .ok_or_else(|| eyre::eyre!("Requested fork `{}` does not exit", id))?;

        // we still need to roll to the transaction, but we only need an empty dummy state since we
        // don't need to update the active journaled state yet
        let mut journaled_state = self.inner.new_journaled_state();

        let mut context = EvmContext {
            block: &mut env.block,
            tx: &mut env.tx,
            cfg: &mut env.cfg,
            journaled_state: &mut journaled_state,
            chain_context,
        };

        self.roll_fork_to_transaction(Some(id), transaction, &mut context)?;
        Ok(id)
    }

    /// Select an existing fork by id.
    /// When switching forks we copy the shared state
    fn select_fork(
        &mut self,
        id: LocalForkId,
        context: &mut EvmContext<'_, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> eyre::Result<()> {
        trace!(?id, "select fork");
        if self.is_active_fork(id) {
            // nothing to do
            return Ok(());
        }

        // Update block number and timestamp of active fork (if any) with current env values,
        // in order to preserve values changed by using `roll` and `warp` cheatcodes.
        if let Some(active_fork_id) = self.active_fork_id() {
            self.forks.update_block(
                self.ensure_fork_id(active_fork_id).cloned()?,
                context.block.number(),
                context.block.timestamp(),
            )?;
        }

        let fork_id = self.ensure_fork_id(id).cloned()?;
        let idx = self.inner.ensure_fork_index(&fork_id)?;
        let fork_env = self
            .forks
            .get_env(fork_id)?
            .ok_or_else(|| eyre::eyre!("Requested fork `{}` does not exit", id))?;

        // If we're currently in forking mode we need to update the journaled_state to this point,
        // this ensures the changes performed while the fork was active are recorded
        if let Some(active) = self.active_fork_mut() {
            active.journaled_state = context.journaled_state.clone();

            let caller = context.tx.caller();
            let caller_account = active.journaled_state.state.get(&context.tx.caller()).cloned();
            let target_fork = self.inner.get_fork_mut(idx);

            // depth 0 will be the default value when the fork was created
            if target_fork.journaled_state.depth == 0 {
                // Initialize caller with its fork info
                if let Some(mut acc) = caller_account {
                    let fork_account = Database::basic(&mut target_fork.db, caller)?
                        .ok_or(BackendError::MissingAccount(caller))?;

                    acc.info = fork_account;
                    target_fork.journaled_state.state.insert(caller, acc);
                }
            }
        } else {
            // this is the first time a fork is selected. This means up to this point all changes
            // are made in a single `JournaledState`, for example after a `setup` that only created
            // different forks. Since the `JournaledState` is valid for all forks until the
            // first fork is selected, we need to update it for all forks and use it as init state
            // for all future forks

            self.set_init_journaled_state(context.journaled_state.clone());
            self.prepare_init_journal_state()?;

            // Make sure that the next created fork has a depth of 0.
            self.fork_init_journaled_state.depth = 0;
        }

        {
            // update the shared state and track
            let mut fork = self.inner.take_fork(idx);

            // Make sure all persistent accounts on the newly selected fork reflect same state as
            // the active db / previous fork.
            // This can get out of sync when multiple forks are created on test `setUp`, then a
            // fork is selected and persistent contract is changed. If first action in test is to
            // select a different fork, then the persistent contract state won't reflect changes
            // done in `setUp` for the other fork.
            // See <https://github.com/foundry-rs/foundry/issues/10296> and <https://github.com/foundry-rs/foundry/issues/10552>.
            let persistent_accounts = self.inner.persistent_accounts.clone();
            if let Some(db) = self.active_fork_db_mut() {
                for addr in persistent_accounts {
                    let Ok(db_account) = db.load_account(addr) else { continue };

                    let Some(fork_account) = fork.journaled_state.state.get_mut(&addr) else {
                        continue;
                    };

                    for (key, val) in &db_account.storage {
                        if let Some(fork_storage) = fork_account.storage.get_mut(key) {
                            fork_storage.present_value = *val;
                        }
                    }
                }
            }

            // since all forks handle their state separately, the depth can drift
            // this is a handover where the target fork starts at the same depth where it was
            // selected. This ensures that there are no gaps in depth which would
            // otherwise cause issues with the tracer
            fork.journaled_state.depth = context.journaled_state.depth;

            // another edge case where a fork is created and selected during setup with not
            // necessarily the same caller as for the test, however we must always
            // ensure that fork's state contains the current sender
            let caller = context.tx.caller();
            fork.journaled_state.state.entry(caller).or_insert_with(|| {
                let caller_account = context
                    .journaled_state
                    .state
                    .get(&context.tx.caller())
                    .map(|acc| acc.info.clone())
                    .unwrap_or_default();

                if !fork.db.cache.accounts.contains_key(&caller) {
                    // update the caller account which is required by the evm
                    fork.db.insert_account_info(caller, caller_account.clone());
                }
                caller_account.into()
            });

            self.update_fork_db(context.journaled_state, &mut fork);

            // insert the fork back
            self.inner.set_fork(idx, fork);
        }

        self.active_fork_ids = Some((id, idx));
        // Update current environment with environment of newly selected fork.
        update_current_env_with_fork_env(context, fork_env);

        Ok(())
    }

    /// This is effectively the same as [`Self::create_select_fork()`] but updating an existing
    /// [`ForkId`] that is mapped to the [`LocalForkId`]
    fn roll_fork(
        &mut self,
        id: Option<LocalForkId>,
        block_number: u64,
        context: &mut EvmContext<'_, BlockT, TxT, HardforkT, ChainContextT>,
    ) -> eyre::Result<()> {
        trace!(?id, ?block_number, "roll fork");
        let id = self.ensure_fork(id)?;
        let (fork_id, backend, fork_env) =
            self.forks.roll_fork(self.inner.ensure_fork_id(id).cloned()?, block_number)?;
        // this will update the local mapping
        self.inner.roll_fork(id, fork_id, backend)?;

        if let Some((active_id, active_idx)) = self.active_fork_ids {
            // the currently active fork is the targeted fork of this call
            if active_id == id {
                // need to update the block's env settings right away, which is otherwise set when
                // forks are selected `select_fork`
                update_current_env_with_fork_env(context, fork_env);

                // we also need to update the journaled_state right away, this has essentially the
                // same effect as selecting (`select_fork`) by discarding
                // non-persistent storage from the journaled_state. This which will
                // reset cached state from the previous block
                let mut persistent_addrs = self.inner.persistent_accounts.clone();
                // we also want to copy the caller state here
                persistent_addrs.extend(self.caller_address());

                let active = self.inner.get_fork_mut(active_idx);
                active.journaled_state = self.fork_init_journaled_state.clone();

                active.journaled_state.depth = context.journaled_state.depth;
                for addr in persistent_addrs {
                    merge_journaled_state_data(
                        addr,
                        context.journaled_state,
                        &mut active.journaled_state,
                    );
                }

                // Ensure all previously loaded accounts are present in the journaled state to
                // prevent issues in the new journalstate, e.g. assumptions that accounts are loaded
                // if the account is not touched, we reload it, if it's touched we clone it.
                //
                // Special case for accounts that are not created: we don't merge their state but
                // load it in order to reflect their state at the new block (they should explicitly
                // be marked as persistent if it is desired to keep state between fork rolls).
                for (addr, acc) in context.journaled_state.state.iter() {
                    if acc.is_created() {
                        if acc.is_touched() {
                            merge_journaled_state_data(
                                *addr,
                                context.journaled_state,
                                &mut active.journaled_state,
                            );
                        }
                    } else {
                        let _ = active.journaled_state.load_account(&mut active.db, *addr);
                    }
                }

                *context.journaled_state = active.journaled_state.clone();
            }
        }
        Ok(())
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
        trace!(?id, ?transaction, "roll fork to transaction");
        let id = self.ensure_fork(id)?;

        let (fork_block, block) =
            self.get_block_number_and_block_for_transaction(id, transaction)?;

        // roll the fork to the transaction's parent block or latest if it's pending, because we
        // need to fork off the parent block's state for tx level forking and then replay the txs
        // before the tx in that block to get the state at the tx
        self.roll_fork(Some(id), fork_block, context)?;

        // we need to update the env to the block
        update_env_block(context.block, &block, context.cfg.spec.into());

        // after we forked at the fork block we need to properly update the block env to the block
        // env of the tx's block
        let _ =
            self.forks.update_block_env(self.inner.ensure_fork_id(id).cloned()?, context.block.clone());

        let env_with_chain =
            EvmEnvWithChainContext::new(context.to_owned_env(), context.chain_context.clone());

        // replay all transactions that came before
        self.replay_until(id, transaction, env_with_chain, context.journaled_state)?;

        Ok(())
    }

    fn transact(
        &mut self,
        maybe_id: Option<LocalForkId>,
        transaction: B256,
        inspector: &mut dyn CheatcodeInspectorTr<
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
        mut env: EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT>,
        journaled_state: &mut JournalInner<JournalEntry>,
    ) -> eyre::Result<()>
    {
        trace!(?maybe_id, ?transaction, "execute transaction");
        let persistent_accounts = self.inner.persistent_accounts.clone();
        let id = self.ensure_fork(maybe_id)?;
        let fork_id = self.ensure_fork_id(id).cloned()?;

        let tx = {
            let fork = self.inner.get_fork_by_id_mut(id)?;
            fork.db.db.get_transaction(transaction)?
        };

        // This is a bit ambiguous because the user wants to transact an arbitrary transaction in
        // the current context, but we're assuming the user wants to transact the transaction as it
        // was mined. Usually this is used in a combination of a fork at the transaction's parent
        // transaction in the block and then the transaction is transacted:
        // <https://github.com/foundry-rs/foundry/issues/6538>
        // So we modify the env to match the transaction's block.
        let (_fork_block, block) =
            self.get_block_number_and_block_for_transaction(id, transaction)?;
        update_env_block(&mut env.block, &block, env.cfg.spec.into());
        let env = self.env_with_handler_cfg(env);

        let fork = self.inner.get_fork_by_id_mut(id)?;
        commit_transaction(
            &tx,
            env,
            journaled_state,
            fork,
            &fork_id,
            &persistent_accounts,
            inspector,
        )
    }

    fn active_fork_id(&self) -> Option<LocalForkId> {
        self.active_fork_ids.map(|(id, _)| id)
    }

    fn active_fork_url(&self) -> Option<String> {
        let fork = self.inner.issued_local_fork_ids.get(&self.active_fork_id()?)?;
        self.forks.get_fork_url(fork.clone()).ok()?
    }

    fn ensure_fork(&self, id: Option<LocalForkId>) -> eyre::Result<LocalForkId> {
        if let Some(id) = id {
            if self.inner.issued_local_fork_ids.contains_key(&id) {
                return Ok(id);
            }
            eyre::bail!("Requested fork `{}` does not exit", id)
        }
        if let Some(id) = self.active_fork_id() { Ok(id) } else { eyre::bail!("No fork active") }
    }

    fn ensure_fork_id(&self, id: LocalForkId) -> eyre::Result<&ForkId> {
        self.inner.ensure_fork_id(id)
    }

    fn diagnose_revert(
        &self,
        callee: Address,
        journaled_state: &JournaledState,
    ) -> Option<RevertDiagnostic> {
        let active_id = self.active_fork_id()?;
        let active_fork = self.active_fork()?;

        if self.inner.forks.len() == 1 {
            // we only want to provide additional diagnostics here when in multifork mode with > 1
            // forks
            return None;
        }

        if !active_fork.is_contract(callee) && !is_contract_in_state(journaled_state, callee) {
            // no contract for `callee` available on current fork, check if available on other forks
            let mut available_on = Vec::new();
            for (id, fork) in self.inner.forks_iter().filter(|(id, _)| *id != active_id) {
                trace!(?id, address=?callee, "checking if account exists");
                if fork.is_contract(callee) {
                    available_on.push(id);
                }
            }

            return if available_on.is_empty() {
                Some(RevertDiagnostic::ContractDoesNotExist {
                    contract: callee,
                    active: active_id,
                    persistent: self.is_persistent(&callee),
                })
            } else {
                // likely user error: called a contract that's not available on active fork but is
                // present other forks
                Some(RevertDiagnostic::ContractExistsOnOtherForks {
                    contract: callee,
                    active: active_id,
                    available_on,
                })
            };
        }
        None
    }

    /// Loads the account allocs from the given `allocs` map into the passed [`JournaledState`].
    ///
    /// Returns [Ok] if all accounts were successfully inserted into the journal, [Err] otherwise.
    fn load_allocs(
        &mut self,
        allocs: &BTreeMap<Address, GenesisAccount>,
        journaled_state: &mut JournaledState,
    ) -> Result<(), BackendError> {
        // Loop through all of the allocs defined in the map and commit them to the journal.
        for (addr, acc) in allocs {
            self.clone_account(acc, addr, journaled_state)?;
        }

        Ok(())
    }

    /// Copies bytecode, storage, nonce and balance from the given genesis account to the target
    /// address.
    ///
    /// Returns [Ok] if data was successfully inserted into the journal, [Err] otherwise.
    fn clone_account(
        &mut self,
        source: &GenesisAccount,
        target: &Address,
        journaled_state: &mut JournaledState,
    ) -> Result<(), BackendError> {
        // Fetch the account from the journaled state. Will create a new account if it does
        // not already exist.
        let mut state_acc = journaled_state.load_account(self, *target)?;

        // Set the account's bytecode and code hash, if the `bytecode` field is present.
        if let Some(bytecode) = source.code.as_ref() {
            state_acc.info.code_hash = keccak256(bytecode);
            let bytecode = Bytecode::new_raw(bytecode.0.clone().into());
            state_acc.info.code = Some(bytecode);
        }

        // Set the account's storage, if the `storage` field is present.
        if let Some(storage) = source.storage.as_ref() {
            state_acc.storage = storage
                .iter()
                .map(|(slot, value)| {
                    let slot = U256::from_be_bytes(slot.0);
                    (
                        slot,
                        EvmStorageSlot::new_changed(
                            state_acc
                                .storage
                                .get(&slot)
                                .map(|s| s.present_value)
                                .unwrap_or_default(),
                            U256::from_be_bytes(value.0),
                            0,
                        ),
                    )
                })
                .collect();
        }
        // Set the account's nonce and balance.
        state_acc.info.nonce = source.nonce.unwrap_or_default();
        state_acc.info.balance = source.balance;

        // Touch the account to ensure the loaded information persists if called in `setUp`.
        journaled_state.touch(*target);

        Ok(())
    }

    fn add_persistent_account(&mut self, account: Address) -> bool {
        trace!(?account, "add persistent account");
        self.inner.persistent_accounts.insert(account)
    }

    fn remove_persistent_account(&mut self, account: &Address) -> bool {
        trace!(?account, "remove persistent account");
        self.inner.persistent_accounts.remove(account)
    }

    fn is_persistent(&self, acc: &Address) -> bool {
        self.inner.persistent_accounts.contains(acc)
    }

    fn allow_cheatcode_access(&mut self, account: Address) -> bool {
        trace!(?account, "allow cheatcode access");
        self.inner.cheatcode_access_accounts.insert(account)
    }

    fn revoke_cheatcode_access(&mut self, account: &Address) -> bool {
        trace!(?account, "revoke cheatcode access");
        self.inner.cheatcode_access_accounts.remove(account)
    }

    fn has_cheatcode_access(&self, account: &Address) -> bool {
        self.inner.cheatcode_access_accounts.contains(account)
    }

    #[inline(always)]
    fn record_cheatcode_purity(&mut self, cheatcode_name: &'static str, is_pure: bool) {
        if !is_pure {
            self.inner.impure_cheatcodes.insert(Cow::Borrowed(cheatcode_name));
        }
    }

    fn set_blockhash(&mut self, block_number: U256, block_hash: B256) {
        if let Some(db) = self.active_fork_db_mut() {
            db.cache.block_hashes.insert(block_number.saturating_to(), block_hash);
        } else {
            self.mem_db.cache.block_hashes.insert(block_number.saturating_to(), block_hash);
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
    for Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>
{
    type Error = DatabaseError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(db) = self.active_fork_db() {
            db.basic_ref(address)
        } else {
            Ok(self.mem_db.basic_ref(address)?)
        }
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        if let Some(db) = self.active_fork_db() {
            db.code_by_hash_ref(code_hash)
        } else {
            Ok(self.mem_db.code_by_hash_ref(code_hash)?)
        }
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        if let Some(db) = self.active_fork_db() {
            DatabaseRef::storage_ref(db, address, index)
        } else {
            Ok(DatabaseRef::storage_ref(&self.mem_db, address, index)?)
        }
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        if let Some(db) = self.active_fork_db() {
            db.block_hash_ref(number)
        } else {
            Ok(self.mem_db.block_hash_ref(number)?)
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
    > DatabaseCommit
    for Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>
{
    fn commit(&mut self, changes: Map<Address, Account>) {
        if let Some(db) = self.active_fork_db_mut() {
            db.commit(changes);
        } else {
            self.mem_db.commit(changes);
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
    > Database
    for Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>
{
    type Error = DatabaseError;
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(db) = self.active_fork_db_mut() {
            Ok(db.basic(address)?)
        } else {
            Ok(self.mem_db.basic(address)?)
        }
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        if let Some(db) = self.active_fork_db_mut() {
            Ok(db.code_by_hash(code_hash)?)
        } else {
            Ok(self.mem_db.code_by_hash(code_hash)?)
        }
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        if let Some(db) = self.active_fork_db_mut() {
            Ok(Database::storage(db, address, index)?)
        } else {
            Ok(Database::storage(&mut self.mem_db, address, index)?)
        }
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        if let Some(db) = self.active_fork_db_mut() {
            Ok(db.block_hash(number)?)
        } else {
            Ok(self.mem_db.block_hash(number)?)
        }
    }
}

/// Variants of a [`revm::Database`]
#[derive(Clone, Debug)]
pub enum BackendDatabaseSnapshot {
    /// Simple in-memory [`revm::Database`]
    InMemory(FoundryEvmInMemoryDB),
    /// Contains the entire forking mode database
    Forked(LocalForkId, ForkId, ForkLookupIndex, Box<Fork>),
}

/// If re-executing the counter example is not guaranteed to yield the same
/// results, this struct contains the reason why.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndeterminismReasons {
    /// Indeterminism due to specifying a fork url without a fork block number
    /// in the test runner config.
    pub global_fork_latest: bool,
    /// The list of executed impure cheatcode signatures. We collect function
    /// signatures instead of function names as whether a cheatcode is impure
    /// can depend on the arguments it takes (e.g. `createFork` without a second
    /// argument means implicitly fork from “latest”). Example signature:
    /// `function createSelectFork(string calldata urlOrAlias) external returns
    /// (uint256 forkId);`.
    ///
    /// The cheatcode signatures are `'static` when created from Rust, but we
    /// need owned deserialization, so we wrap it in a `Cow`.
    /// Using `Cow` for owned deserialization.
    pub impure_cheatcodes: HashSet<Cow<'static, str>>,
}

impl IndeterminismReasons {
    pub fn merge(&mut self, other: Option<IndeterminismReasons>) {
        if let Some(other) = other {
            self.global_fork_latest = self.global_fork_latest || other.global_fork_latest;
            self.impure_cheatcodes = self.impure_cheatcodes.union(&other.impure_cheatcodes).cloned().collect();
        }
    }
}

/// Represents a fork
#[derive(Clone, Debug)]
pub struct Fork {
    db: ForkDB,
    journaled_state: JournaledState,
    fork_block_number: Option<u64>,
}

// === impl Fork ===

impl Fork {
    /// Returns true if the account is a contract
    pub fn is_contract(&self, acc: Address) -> bool {
        if let Ok(Some(acc)) = self.db.basic_ref(acc)
            && acc.code_hash != KECCAK_EMPTY
        {
            return true;
        }
        is_contract_in_state(&self.journaled_state, acc)
    }
}

/// Container type for various Backend related data
#[derive(Clone, Debug)]
pub struct BackendInner<BlockT, TxT, HardforkT> {
    /// Stores the `ForkId` of the fork the `Backend` launched with from the start.
    ///
    /// In other words if [`Backend::spawn()`] was called with a `CreateFork` command, to launch
    /// directly in fork mode, this holds the corresponding fork identifier of this fork.
    pub launched_with_fork: Option<LaunchedWithFork>,
    /// This tracks numeric fork ids and the `ForkId` used by the handler.
    ///
    /// This is necessary, because there can be multiple `Backends` associated with a single
    /// `ForkId` which is only a pair of endpoint + block. Since an existing fork can be
    /// modified (e.g. `roll_fork`), but this should only affect the fork that's unique for the
    /// test and not the `ForkId`
    ///
    /// This ensures we can treat forks as unique from the context of a test, so rolling to another
    /// is basically creating(or reusing) another `ForkId` that's then mapped to the previous
    /// issued _local_ numeric identifier, that remains constant, even if the underlying fork
    /// backend changes.
    pub issued_local_fork_ids: HashMap<LocalForkId, ForkId>,
    /// tracks all the created forks
    /// Contains the index of the corresponding `ForkDB` in the `forks` vec
    pub created_forks: HashMap<ForkId, ForkLookupIndex>,
    /// Holds all created fork databases
    // Note: data is stored in an `Option` so we can remove it without reshuffling
    pub forks: Vec<Option<Fork>>,
    /// Contains state snapshots made at a certain point
    pub state_snapshots:
        StateSnapshots<BackendStateSnapshot<BackendDatabaseSnapshot, BlockT, TxT, HardforkT>>,
    /// Tracks whether there was a failure in a snapshot that was reverted
    ///
    /// The Test contract contains a bool variable that is set to true when an `assert` function
    /// failed. When a snapshot is reverted, it reverts the state of the evm, but we still want
    /// to know if there was an `assert` that failed after the snapshot was taken so that we can
    /// check if the test function passed all asserts even across snapshots. When a snapshot is
    /// reverted we get the _current_ `revm::JournaledState` which contains the state that we can
    /// check if the `_failed` variable is set,
    /// additionally
    pub has_state_snapshot_failure: bool,
    /// Tracks the address of a Test contract
    ///
    /// This address can be used to inspect the state of the contract when a
    /// test is being executed. E.g. the `_failed` variable of `DSTest`
    pub test_contract_address: Option<Address>,
    /// Tracks the caller of the test function
    pub caller: Option<Address>,
    /// Tracks numeric identifiers for forks
    pub next_fork_id: LocalForkId,
    /// All accounts that should be kept persistent when switching forks.
    /// This means all accounts stored here _don't_ use a separate storage section on each fork
    /// instead the use only one that's persistent across fork swaps.
    pub persistent_accounts: HashSet<Address>,
    /// The configured spec id
    pub spec_id: HardforkT,
    /// All accounts that are allowed to execute cheatcodes
    pub cheatcode_access_accounts: HashSet<Address>,
    /// The set of executed cheatcodes that are impure. The values are the
    /// Solidity method declarations of the cheatcodes.
    pub impure_cheatcodes: HashSet<Cow<'static, str>>,
}

// === impl BackendInner ===

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr>
    BackendInner<BlockT, TxT, HardforkT>
{
    pub fn ensure_fork_id(&self, id: LocalForkId) -> eyre::Result<&ForkId> {
        self.issued_local_fork_ids
            .get(&id)
            .ok_or_else(|| eyre::eyre!("No matching fork found for {}", id))
    }

    pub fn ensure_fork_index(&self, id: &ForkId) -> eyre::Result<ForkLookupIndex> {
        self.created_forks
            .get(id)
            .copied()
            .ok_or_else(|| eyre::eyre!("No matching fork found for {}", id))
    }

    pub fn ensure_fork_index_by_local_id(&self, id: LocalForkId) -> eyre::Result<ForkLookupIndex> {
        self.ensure_fork_index(self.ensure_fork_id(id)?)
    }

    /// Returns the underlying fork mapped to the index
    #[track_caller]
    fn get_fork(&self, idx: ForkLookupIndex) -> &Fork {
        debug_assert!(idx < self.forks.len(), "fork lookup index must exist");
        self.forks.get(idx).and_then(|f| f.as_ref()).expect("fork lookup index must exist")
    }

    /// Returns the underlying fork mapped to the index
    #[track_caller]
    fn get_fork_mut(&mut self, idx: ForkLookupIndex) -> &mut Fork {
        debug_assert!(idx < self.forks.len(), "fork lookup index must exist");
        self.forks.get_mut(idx).and_then(|f| f.as_mut()).expect("fork lookup index must exist")
    }

    /// Returns the underlying fork corresponding to the id
    #[track_caller]
    fn get_fork_by_id_mut(&mut self, id: LocalForkId) -> eyre::Result<&mut Fork> {
        let idx = self.ensure_fork_index_by_local_id(id)?;
        Ok(self.get_fork_mut(idx))
    }

    /// Returns the underlying fork corresponding to the id
    #[track_caller]
    fn get_fork_by_id(&self, id: LocalForkId) -> eyre::Result<&Fork> {
        let idx = self.ensure_fork_index_by_local_id(id)?;
        Ok(self.get_fork(idx))
    }

    /// Removes the fork
    fn take_fork(&mut self, idx: ForkLookupIndex) -> Fork {
        debug_assert!(idx < self.forks.len(), "fork lookup index must exist");
        self.forks.get_mut(idx).and_then(Option::take).expect("fork lookup index must exist")
    }

    fn set_fork(&mut self, idx: ForkLookupIndex, fork: Fork) {
        if let Some(slot) = self.forks.get_mut(idx) {
            *slot = Some(fork);
        }
    }

    /// Returns an iterator over Forks
    pub fn forks_iter(&self) -> impl Iterator<Item = (LocalForkId, &Fork)> + '_ {
        self.issued_local_fork_ids
            .iter()
            .map(|(id, fork_id)| {
                let idx = *self.created_forks.get(fork_id).expect("fork_id must exist");
                (*id, self.get_fork(idx))
            })
    }

    /// Returns a mutable iterator over all Forks
    pub fn forks_iter_mut(&mut self) -> impl Iterator<Item = &mut Fork> + '_ {
        self.forks.iter_mut().filter_map(|f| f.as_mut())
    }

    /// Reverts the entire fork database
    pub fn revert_state_snapshot(
        &mut self,
        id: LocalForkId,
        fork_id: ForkId,
        idx: ForkLookupIndex,
        fork: Fork,
    ) {
        self.created_forks.insert(fork_id.clone(), idx);
        self.issued_local_fork_ids.insert(id, fork_id);
        self.set_fork(idx, fork);
    }

    /// Updates the fork and the local mapping and returns the new index for the `fork_db`
    pub fn update_fork_mapping(
        &mut self,
        id: LocalForkId,
        fork_id: ForkId,
        db: ForkDB,
        journaled_state: JournaledState,
        fork_block_number: Option<u64>,
    ) -> ForkLookupIndex {
        let idx = self.forks.len();
        self.issued_local_fork_ids.insert(id, fork_id.clone());
        self.created_forks.insert(fork_id, idx);

        let fork = Fork { db, journaled_state, fork_block_number };
        self.forks.push(Some(fork));
        idx
    }

    pub fn roll_fork(
        &mut self,
        id: LocalForkId,
        new_fork_id: ForkId,
        backend: SharedBackend,
    ) -> eyre::Result<ForkLookupIndex> {
        let fork_id = self.ensure_fork_id(id)?;
        let idx = self.ensure_fork_index(fork_id)?;

        if let Some(active) = self.forks.get_mut(idx).and_then(|f| f.as_mut()) {
            // we initialize a _new_ `ForkDB` but keep the state of persistent accounts
            let mut new_db = ForkDB::new(backend);
            for addr in self.persistent_accounts.iter().copied() {
                merge_db_account_data(addr, &active.db, &mut new_db);
            }
            active.db = new_db;
        }
        // update mappings
        self.issued_local_fork_ids.insert(id, new_fork_id.clone());
        self.created_forks.insert(new_fork_id, idx);
        Ok(idx)
    }

    /// Inserts a _new_ `ForkDB` and issues a new local fork identifier
    ///
    /// Also returns the index where the `ForDB` is stored
    pub fn insert_new_fork(
        &mut self,
        fork_id: ForkId,
        db: ForkDB,
        journaled_state: JournaledState,
        fork_block_number: Option<u64>,
    ) -> (LocalForkId, ForkLookupIndex) {
        let idx = self.forks.len();
        self.created_forks.insert(fork_id.clone(), idx);
        let id = self.next_id();
        self.issued_local_fork_ids.insert(id, fork_id);
        let fork = Fork { db, journaled_state, fork_block_number };
        self.forks.push(Some(fork));
        (id, idx)
    }

    fn next_id(&mut self) -> U256 {
        let id = self.next_fork_id;
        self.next_fork_id += U256::from(1);
        id
    }

    /// Returns the number of issued ids
    pub fn len(&self) -> usize {
        self.issued_local_fork_ids.len()
    }

    /// Returns true if no forks are issued
    pub fn is_empty(&self) -> bool {
        self.issued_local_fork_ids.is_empty()
    }

    pub fn precompiles(&self) -> &'static Precompiles {
        Precompiles::new(PrecompileSpecId::from_spec_id(self.spec_id.into()))
    }

    /// Returns a new, empty, `JournaledState` with set precompiles
    pub fn new_journaled_state(&self) -> JournaledState {
        let mut journal = {
            let mut journal_inner = JournalInner::new();
            journal_inner.set_spec_id(self.spec_id.into());
            journal_inner
        };
        journal.precompiles.extend(self.precompiles().addresses().copied());
        journal
    }
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr> Default
    for BackendInner<BlockT, TxT, HardforkT>
{
    fn default() -> Self {
        Self {
            launched_with_fork: None,
            issued_local_fork_ids: HashMap::default(),
            created_forks: HashMap::default(),
            forks: vec![],
            state_snapshots: StateSnapshots::default(),
            has_state_snapshot_failure: false,
            test_contract_address: None,
            caller: None,
            next_fork_id: LocalForkId::default(),
            persistent_accounts: HashSet::default(),
            spec_id: HardforkT::default(),
            // grant the cheatcode,default test and caller address access to execute cheatcodes
            // itself
            cheatcode_access_accounts: HashSet::from([
                CHEATCODE_ADDRESS,
                TEST_CONTRACT_ADDRESS,
                CALLER,
            ]),
            impure_cheatcodes: HashSet::default(),
        }
    }
}

/// Holds the initial (global) fork info
#[derive(Clone, Debug)]
pub struct LaunchedWithFork {
    /// The fork id
    pub fork_id: ForkId,
    /// The local fork id
    pub local_fork_id: LocalForkId,
    /// The fork lookup index
    pub fork_look_up_index: ForkLookupIndex,
    /// The fork block number. If `None` that means implicitly "latest".
    pub fork_block_number: Option<u64>,
}

/// This updates the currently used env with the fork's environment
pub(crate) fn update_current_env_with_fork_env<BlockT, TxT, HardforkT, ChainContextT>(
    current: &mut EvmContext<'_, BlockT, TxT, HardforkT, ChainContextT>,
    fork: EvmEnv<BlockT, TxT, HardforkT>,
) where
    TxT: TransactionEnvTr + TransactionEnvMut,
{
    *current.block = fork.block;
    *current.cfg = fork.cfg;
    current.tx.set_chain_id(fork.tx.chain_id());
}

/// Clones the data of the given `accounts` from the `active` database into the `fork_db`
/// This includes the data held in storage (`CacheDB`) and kept in the `JournaledState`.
pub(crate) fn merge_account_data<ExtDB: DatabaseRef>(
    accounts: impl IntoIterator<Item = Address>,
    active: &CacheDB<ExtDB>,
    active_journaled_state: &mut JournaledState,
    target_fork: &mut Fork,
) {
    for addr in accounts {
        merge_db_account_data(addr, active, &mut target_fork.db);
        merge_journaled_state_data(addr, active_journaled_state, &mut target_fork.journaled_state);
    }

    *active_journaled_state = target_fork.journaled_state.clone();
}

/// Clones the account data from the `active_journaled_state`  into the `fork_journaled_state`
fn merge_journaled_state_data(
    addr: Address,
    active_journaled_state: &JournaledState,
    fork_journaled_state: &mut JournaledState,
) {
    if let Some(mut acc) = active_journaled_state.state.get(&addr).cloned() {
        trace!(?addr, "updating journaled_state account data");
        if let Some(fork_account) = fork_journaled_state.state.get_mut(&addr) {
            // This will merge the fork's tracked storage with active storage and update values
            fork_account.storage.extend(std::mem::take(&mut acc.storage));
            // swap them so we can insert the account as whole in the next step
            std::mem::swap(&mut fork_account.storage, &mut acc.storage);
        }
        fork_journaled_state.state.insert(addr, acc);
    }
}

/// Clones the account data from the `active` db into the `ForkDB`
fn merge_db_account_data<ExtDB: DatabaseRef>(
    addr: Address,
    active: &CacheDB<ExtDB>,
    fork_db: &mut ForkDB,
) {
    use revm_primitives::hash_map::Entry;

    trace!(?addr, "merging database data");

    let Some(acc) = active.cache.accounts.get(&addr) else { return };

    // port contract cache over
    if let Some(code) = active.cache.contracts.get(&acc.info.code_hash) {
        trace!("merging contract cache");
        fork_db.cache.contracts.insert(acc.info.code_hash, code.clone());
    }

    // port account storage over
    match fork_db.cache.accounts.entry(addr) {
        Entry::Vacant(vacant) => {
            trace!("target account not present - inserting from active");
            // if the fork_db doesn't have the target account
            // insert the entire thing
            vacant.insert(acc.clone());
        }
        Entry::Occupied(mut occupied) => {
            trace!("target account present - merging storage slots");
            // if the fork_db does have the system,
            // extend the existing storage (overriding)
            let fork_account = occupied.get_mut();
            fork_account.storage.extend(&acc.storage);
        }
    }
}

/// Returns true of the address is a contract
fn is_contract_in_state(journaled_state: &JournaledState, acc: Address) -> bool {
    journaled_state
        .state
        .get(&acc)
        .map(|acc| acc.info.code_hash != KECCAK_EMPTY)
        .unwrap_or_default()
}

/// Updates the env's block with the block's data
fn update_env_block<BlockT: BlockEnvTr>(block_env: &mut BlockT, block: &AnyRpcBlock, spec: SpecId) {
    block_env.set_timestamp(U256::from(block.header.timestamp));
    block_env.set_beneficiary(block.header.beneficiary);
    block_env.set_difficulty(block.header.difficulty);
    block_env.set_prevrandao(Some(block.header.mix_hash.unwrap_or_default()));
    block_env.set_basefee(block.header.base_fee_per_gas.unwrap_or_default());
    block_env.set_gas_limit(block.header.gas_limit);
    block_env.set_number(U256::from(block.header.number));
    if let Some(excess_blob_gas) = block.header.excess_blob_gas {
        block_env.set_blob_excess_gas_and_price(
            excess_blob_gas,
            get_blob_base_fee_update_fraction_by_spec_id(spec),
        );
    }
}

/// Executes the given transaction and commits state changes to the database
/// _and_ the journaled state, with an optional inspector
fn commit_transaction<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
    InspectorT,
>(
    tx: &RpcTransaction<AnyTxEnvelope>,
    mut env: EvmEnvWithChainContext<BlockT, TxT, HardforkT, ChainContextT>,
    journaled_state: &mut JournalInner<JournalEntry>,
    fork: &mut Fork,
    fork_id: &ForkId,
    persistent_accounts: &HashSet<Address>,
    inspector: InspectorT,
) -> eyre::Result<()>
where
    InspectorT: CheatcodeInspectorTr<
        BlockT,
        TxT,
        HardforkT,
        Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>,
        ChainContextT,
    >,
{
    configure_tx_env(&mut env.tx, tx);

    let now = Instant::now();
    let res = {
        let fork = fork.clone();
        let journaled_state = journaled_state.clone();
        let db = Backend::new_with_fork(fork_id, fork, journaled_state)?;
        let tx = std::mem::take(&mut env.tx);

        EvmBuilderT::evm_with_inspector(db, env, inspector)
            .inspect_tx(tx)
            .wrap_err("backend: failed committing transaction")?
    };
    trace!(elapsed = ?now.elapsed(), "transacted transaction");

    apply_state_changeset(res.state, journaled_state, fork, persistent_accounts)?;
    Ok(())
}

/// Helper method which updates data in the state with the data from the database.
/// Does not change state for persistent accounts (for roll fork to transaction and transact).
pub fn update_state<DB: Database>(
    state: &mut EvmState,
    db: &mut DB,
    persistent_accounts: Option<&HashSet<Address>>,
) -> Result<(), DB::Error> {
    for (addr, acc) in state.iter_mut() {
        if !persistent_accounts.is_some_and(|accounts| accounts.contains(addr)) {
            acc.info = db.basic(*addr)?.unwrap_or_default();
            for (key, val) in &mut acc.storage {
                val.present_value = db.storage(*addr, *key)?;
            }
        }
    }

    Ok(())
}

/// Applies the changeset of a transaction to the active journaled state and also commits it in the
/// forked db
fn apply_state_changeset(
    state: Map<revm::primitives::Address, Account>,
    journaled_state: &mut JournaledState,
    fork: &mut Fork,
    persistent_accounts: &HashSet<Address>,
) -> Result<(), DatabaseError> {
    // commit the state and update the loaded accounts
    fork.db.commit(state);

    update_state(&mut journaled_state.state, &mut fork.db, Some(persistent_accounts))?;
    update_state(&mut fork.journaled_state.state, &mut fork.db, Some(persistent_accounts))?;

    Ok(())
}

/// Returns whether the sender is a known L2 system sender that is the first tx
/// in every block.
///
/// Transactions from these senders usually don't have a any fee information.
///
/// See: [`ARBITRUM_SENDER`], [`OPTIMISM_SYSTEM_ADDRESS`]
#[inline]
fn is_known_system_sender(sender: Address) -> bool {
    [ARBITRUM_SENDER, OPTIMISM_SYSTEM_ADDRESS].contains(&sender)
}

#[cfg(test)]
#[cfg(feature = "test-remote")]
mod tests {
    use std::collections::BTreeSet;

    use alloy_chains::NamedChain;
    use alloy_primitives::{Address, U256};
    use alloy_provider::Provider;
    use edr_test_utils::env::get_alchemy_url;
    use foundry_fork_db::cache::{BlockchainDb, BlockchainDbMeta};
    use revm::{
        context::{
            result::{HaltReason, InvalidTransaction},
            BlockEnv, TxEnv,
        },
        primitives::hardfork::SpecId,
        DatabaseRef,
    };
    use tempfile::tempdir;

    use crate::{
        backend::Backend,
        evm_context::L1EvmBuilder,
        fork::{provider::get_http_provider, CreateFork},
        opts::EvmOpts,
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn can_read_write_cache() {
        let endpoint = get_alchemy_url();
        let cache_dir = tempdir().expect("Should create tempdir");

        let provider = get_http_provider(&endpoint);

        let block_num = provider.get_block_number().await.unwrap();

        let evm_opts =
            EvmOpts::<SpecId> { fork_block_number: Some(block_num), ..EvmOpts::default() };

        let (env, _block) = evm_opts.fork_evm_env(&endpoint).await.unwrap();

        let fork = CreateFork {
            rpc_cache_path: Some(cache_dir.path().into()),
            url: endpoint,
            env: env.clone(),
            evm_opts,
        };

        let backend = Backend::<
            BlockEnv,
            TxEnv,
            L1EvmBuilder,
            HaltReason,
            SpecId,
            InvalidTransaction,
            (),
        >::spawn(Some(fork.clone()), Vec::default()).unwrap();

        // some rng contract from etherscan
        let address: Address = "63091244180ae240c87d1f528f5f269134cb07b3".parse().unwrap();

        let idx = U256::from(0u64);
        let _value = backend.storage_ref(address, idx);
        let _account = backend.basic_ref(address);

        // fill some slots
        let num_slots = 10u64;
        for idx in 1..num_slots {
            let _ = backend.storage_ref(address, U256::from(idx));
        }
        drop(backend);

        let meta =
            BlockchainDbMeta { chain: None, block_env: env.block, hosts: BTreeSet::default() };

        let db = BlockchainDb::new(
            meta,
            Some(
                fork.block_cache_dir(NamedChain::Mainnet, block_num)
                    .expect("Rpc cache path should be set"),
            ),
        );
        assert!(db.accounts().read().contains_key(&address));
        assert!(db.storage().read().contains_key(&address));
        assert_eq!(db.storage().read().get(&address).unwrap().len(), num_slots as usize);
    }
}
