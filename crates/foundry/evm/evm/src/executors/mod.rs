//! EVM executor abstractions, which can execute calls.
//!
//! Used for running tests, scripts, and interacting with the inner backend
//! which holds the state.

// TODO: The individual executors in this module should be moved into the
// respective crates, and the `Executor` struct should be accessed using a trait
// defined in `foundry-evm-core` instead of the concrete `Executor` type.

use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

use alloy_dyn_abi::{DynSolValue, FunctionExt, JsonAbiExt};
use alloy_json_abi::Function;
use alloy_primitives::{keccak256, map::{AddressHashMap, HashMap}, Address, Bytes, Log, TxKind, U256};
use alloy_sol_types::{sol, SolCall};
use derive_where::derive_where;
use foundry_evm_core::{
    backend::{Backend, BackendError, BackendResult, CheatcodeBackend, CowBackend},
    constants::{
        CALLER, CHEATCODE_ADDRESS, DEFAULT_CREATE2_DEPLOYER, DEFAULT_CREATE2_DEPLOYER_CODE,
    },
    decode::RevertDecoder,
    evm_context::{EvmBuilderTrait, TransactionErrorTrait},
    utils::StateChangeset,
};
use foundry_evm_coverage::HitMaps;
use foundry_evm_traces::{SparsedTraceArena, TracingMode};
use revm::{
    bytecode::Bytecode,
    context::result::{ExecutionResult, HaltReason, HaltReasonTr, ResultAndState},
    context_interface::result::Output,
    database::{DatabaseCommit, DatabaseRef},
    interpreter::{return_ok, InstructionResult},
};
use revm::context::transaction::SignedAuthorization;
use crate::inspectors::{Cheatcodes, InspectorData, InspectorStack};

mod builder;
pub use builder::{ExecutorBuilder, ExecutorBuilderError};
use foundry_evm_core::evm_context::{
    BlockEnvTr, ChainContextTr, EvmEnv, HardforkTr, TransactionEnvTr,
};
// Leaving this intentionally removed as it was flagged as unused
use foundry_evm_core::{backend::IndeterminismReasons, decode::SkipReason};
use foundry_evm_core::backend::GLOBAL_FAIL_SLOT;
use foundry_evm_core::constants::{CHEATCODE_CONTRACT_HASH, DEFAULT_CREATE2_DEPLOYER_DEPLOYER};

pub mod fuzz;
pub use fuzz::FuzzedExecutor;

pub mod invariant;
pub mod stack_trace;

pub use invariant::InvariantExecutor;

sol! {
    interface ITest {
        function setUp() external;
        function failed() external view returns (bool failed);

        #[derive(Default)]
        function beforeTestSetup(bytes4 testSelector) public view returns (bytes[] memory beforeTestCalldata);
    }
}

/// EVM executor.
///
/// The executor can be configured with various `revm::Inspector`s, like `Cheatcodes`.
///
/// There are multiple ways of interacting the EVM:
/// - `call`: executes a transaction, but does not persist any state changes; similar to `eth_call`,
///   where the EVM state is unchanged after the call.
/// - `transact`: executes a transaction and persists the state changes
/// - `deploy`: a special case of `transact`, specialized for persisting the state of a contract
///   deployment
/// - `setup`: a special case of `transact`, used to set up the environment for a test
#[derive(Clone, Debug)]
pub struct Executor<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
> {
    /// The underlying `revm::Database` that contains the EVM storage.
    // Note: We do not store an EVM here, since we are really
    // only interested in the database. REVM's `EVM` is a thin
    // wrapper around spawning a new EVM on every call anyway,
    // so the performance difference should be negligible.
    backend: Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>,
    /// The EVM environment.
    env: EvmEnv<BlockT, TxT, HardforkT>,
    /// The Revm inspector stack.
    inspector: InspectorStack<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>,
    chain_context: ChainContextT,
    /// The gas limit for calls and deployments.
    gas_limit: u64,
}

impl<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
> Executor<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT> {
    /// Creates a new `ExecutorBuilder`.
    #[inline]
    pub fn builder() -> ExecutorBuilder<BlockT, TxT, HardforkT, ChainContextT> {
        ExecutorBuilder::new()
    }

    /// Creates a new `Executor` with the given arguments.
    #[inline]
    pub fn new(
        mut backend: Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>,
        env: EvmEnv<BlockT, TxT, HardforkT>,
        chain_context: ChainContextT,
        inspector: InspectorStack<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>,
        gas_limit: u64,
    ) -> Self {
        // Need to create a non-empty contract on the cheatcodes address so `extcodesize` checks
        // do not fail.
        backend.insert_account_info(
            CHEATCODE_ADDRESS,
            revm::state::AccountInfo {
                code: Some(Bytecode::new_raw(Bytes::from_static(&[0]))),
                // Also set the code hash manually so that it's not computed later.
                // The code hash value does not matter, as long as it's not zero or `KECCAK_EMPTY`.
                code_hash: CHEATCODE_CONTRACT_HASH,
                ..Default::default()
            },
        );

        Self { backend, env, inspector, chain_context, gas_limit }
    }

    /// Returns a reference to the EVM environment.
    pub fn env(&self) -> &EvmEnv<BlockT, TxT, HardforkT> {
        &self.env
    }

    /// Returns a mutable reference to the EVM environment.
    pub fn env_mut(&mut self) -> &mut EvmEnv<BlockT, TxT, HardforkT> {
        &mut self.env
    }

    /// Returns a reference to the EVM inspector.
    pub fn inspector(&self) -> &InspectorStack<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT> {
        &self.inspector
    }

    /// Returns a mutable reference to the EVM inspector.
    pub fn inspector_mut(&mut self) -> &mut InspectorStack<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT> {
        &mut self.inspector
    }
    
    /// Returns the EVM spec ID.
    pub fn spec_id(&self) -> HardforkT {
        self.env.cfg.spec
    }

    /// Sets the EVM spec ID.
    pub fn set_spec_id(&mut self, spec_id: HardforkT) {
        self.env.cfg.spec = spec_id;
    }

    /// Returns the gas limit for calls and deployments.
    ///
    /// This is different from the gas limit imposed by the passed in environment, as those limits
    /// are used by the EVM for certain opcodes like `gaslimit`.
    pub fn gas_limit(&self) -> u64 {
        self.gas_limit
    }

    /// Sets the gas limit for calls and deployments.
    pub fn set_gas_limit(&mut self, gas_limit: u64) {
        self.gas_limit = gas_limit;
    }

    #[inline]
    pub fn set_tracing(&mut self, mode: TracingMode) -> &mut Self {
        self.inspector_mut().tracing(mode);
        self
    }
    
    /// Whether tracing is on and if it records EVM step level data.
    pub fn tracer_records_steps(&self) -> bool {
        self.inspector().tracer.as_ref().is_some_and(|tracer| tracer.config().record_steps)
    }

    /// Whether when re-executing the calls the same results are guaranteed.
    pub fn safe_to_re_execute(&self) -> bool {
        self.backend.safe_to_re_execute()
    }
}

impl<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: 'static + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: 'static + HaltReasonTr + TryInto<HaltReason>,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: 'static + ChainContextTr,
> Executor<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT> {
    /// Returns a reference to the EVM backend.
    pub fn backend(&self) -> &Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT> {
        &self.backend
    }

    /// Returns a mutable reference to the EVM backend.
    pub fn backend_mut(&mut self) -> &mut Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT> {
        &mut self.backend
    }

    /// Set the balance of an account.
    pub fn set_balance(&mut self, address: Address, amount: U256) -> BackendResult<()> {
        trace!(?address, ?amount, "setting account balance");
        let mut account = self.backend().basic_ref(address)?.unwrap_or_default();
        account.balance = amount;
        self.backend_mut().insert_account_info(address, account);
        Ok(())
    }

    /// Gets the balance of an account
    pub fn get_balance(&self, address: Address) -> BackendResult<U256> {
        Ok(self.backend().basic_ref(address)?.map(|acc| acc.balance).unwrap_or_default())
    }

    /// Set the nonce of an account.
    pub fn set_nonce(&mut self, address: Address, nonce: u64) -> BackendResult<()> {
        let mut account = self.backend().basic_ref(address)?.unwrap_or_default();
        account.nonce = nonce;
        self.backend_mut().insert_account_info(address, account);
        self.env_mut().tx.set_nonce(nonce);
        Ok(())
    }

    /// Returns the nonce of an account.
    pub fn get_nonce(&self, address: Address) -> BackendResult<u64> {
        Ok(self.backend().basic_ref(address)?.map(|acc| acc.nonce).unwrap_or_default())
    }

    /// Set the code of an account.
    pub fn set_code(&mut self, address: Address, code: Bytecode) -> BackendResult<()> {
        let mut account = self.backend().basic_ref(address)?.unwrap_or_default();
        account.code_hash = keccak256(code.original_byte_slice());
        account.code = Some(code);
        self.backend_mut().insert_account_info(address, account);
        Ok(())
    }

    /// Set the storage of an account.
    pub fn set_storage(
        &mut self,
        address: Address,
        storage: HashMap<U256, U256>,
    ) -> BackendResult<()> {
        self.backend_mut().replace_account_storage(address, storage)?;
        Ok(())
    }

    /// Set a storage slot of an account.
    pub fn set_storage_slot(
        &mut self,
        address: Address,
        slot: U256,
        value: U256,
    ) -> BackendResult<()> {
        self.backend_mut().insert_account_storage(address, slot, value)?;
        Ok(())
    }

    /// Returns `true` if the account has no code.
    pub fn is_empty_code(&self, address: Address) -> BackendResult<bool> {
        Ok(self.backend().basic_ref(address)?.map(|acc| acc.is_empty_code_hash()).unwrap_or(true))
    }

    /// Creates the default CREATE2 Contract Deployer for local tests and scripts.
    pub fn deploy_create2_deployer(&mut self) -> eyre::Result<()> {
        trace!("deploying local create2 deployer");
        let create2_deployer_account = self
            .backend()
            .basic_ref(DEFAULT_CREATE2_DEPLOYER)?
            .ok_or_else(|| BackendError::MissingAccount(DEFAULT_CREATE2_DEPLOYER))?;

        // If the deployer is not currently deployed, deploy the default one.
        if create2_deployer_account.code.is_none_or(|code| code.is_empty()) {
            let creator = DEFAULT_CREATE2_DEPLOYER_DEPLOYER;

            // Probably 0, but just in case.
            let initial_balance = self.get_balance(creator)?;
            self.set_balance(creator, U256::MAX)?;

            let res =
                self.deploy(creator, DEFAULT_CREATE2_DEPLOYER_CODE.into(), U256::ZERO, None)?;
            trace!(create2=?res.address, "deployed local create2 deployer");

            self.set_balance(creator, initial_balance)?;
        }
        Ok(())
    }

    /// Deploys a contract and commits the new state to the underlying database.
    ///
    /// Executes a CREATE transaction with the contract `code` and persistent database state
    /// modifications.
    pub fn deploy(
        &mut self,
        from: Address,
        code: Bytes,
        value: U256,
        rd: Option<&RevertDecoder>,
    ) -> Result<DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
        let env = self.build_test_env(from, TxKind::Create, code, value);
        self.deploy_with_env(env, rd)
    }

    /// Deploys a contract using the given `env` and commits the new state to the underlying
    /// database.
    ///
    /// # Panics
    ///
    /// Panics if `env.tx.kind` is not `TxKind::Create(_)`.
    #[instrument(name = "deploy", level = "debug", skip_all)]
    pub fn deploy_with_env(
        &mut self,
        env: EvmEnv<BlockT, TxT, HardforkT>,
        rd: Option<&RevertDecoder>,
    ) -> Result<DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
        assert!(
            matches!(env.tx.kind(), TxKind::Create),
            "Expected create transaction, got {:?}",
            env.tx.kind()
        );
        trace!(sender=%env.tx.caller(), "deploying contract");

        let mut result = self.transact_with_env(env)?;
        result = result.into_result(rd)?;
        let Some(Output::Create(_, Some(address))) = result.out else {
            panic!("Deployment succeeded, but no address was returned: {result:#?}");
        };

        // also mark this library as persistent, this will ensure that the state of the library is
        // persistent across fork swaps in forking mode
        self.backend_mut().add_persistent_account(address);

        debug!(%address, "deployed contract");

        Ok(DeployResult { raw: result, address })
    }

    /// Calls the `setUp()` function on a contract.
    ///
    /// This will commit any state changes to the underlying database.
    ///
    /// Ayn changes made during the setup call to env's block environment are persistent, for
    /// example `vm.chainId()` will change the `block.chainId` for all subsequent test calls.
    #[instrument(name = "setup", level = "debug", skip_all)]
    pub fn setup(
        &mut self,
        from: Option<Address>,
        to: Address,
        rd: Option<&RevertDecoder>,
    ) -> Result<RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
        trace!(?from, ?to, "setting up contract");

        let from = from.unwrap_or(CALLER);
        self.backend_mut().set_test_contract(to).set_caller(from);
        let calldata = Bytes::from_static(&ITest::setUpCall::SELECTOR);
        let mut res = self.transact_raw(from, to, calldata, U256::ZERO)?;
        res = res.into_result(rd)?;

        // record any changes made to the block's environment during setup
        self.env_mut().block = res.env.block.clone();
        // and also the chainid, which can be set manually
        self.env_mut().cfg.chain_id = res.env.cfg.chain_id;

        let success =
            self.is_raw_call_success(to, Cow::Borrowed(&res.state_changeset), &res, false);
        if !success {
            return Err(res.into_execution_error("execution error".to_string()).into());
        }

        Ok(res)
    }

    /// Performs a call to an account on the current state of the VM.
    pub fn call(
        &self,
        from: Address,
        to: Address,
        func: &Function,
        args: &[DynSolValue],
        value: U256,
        rd: Option<&RevertDecoder>,
    ) -> Result<CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
        let calldata = Bytes::from(func.abi_encode_input(args)?);
        let result = self.call_raw(from, to, calldata, value)?;
        result.into_decoded_result(func, rd)
    }

    /// Performs a call to an account on the current state of the VM.
    pub fn call_sol<C: SolCall>(
        &self,
        from: Address,
        to: Address,
        args: &C,
        value: U256,
        rd: Option<&RevertDecoder>,
    ) -> Result<CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, C::Return>, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
        let calldata = Bytes::from(args.abi_encode());
        let mut raw = self.call_raw(from, to, calldata, value)?;
        raw = raw.into_result(rd)?;
        Ok(CallResult { decoded_result: C::abi_decode_returns(&raw.result)?, raw })
    }

    /// Performs a call to an account on the current state of the VM.
    pub fn transact(
        &mut self,
        from: Address,
        to: Address,
        func: &Function,
        args: &[DynSolValue],
        value: U256,
        rd: Option<&RevertDecoder>,
    ) -> Result<CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
        let calldata = Bytes::from(func.abi_encode_input(args)?);
        let result = self.transact_raw(from, to, calldata, value)?;
        result.into_decoded_result(func, rd)
    }

    /// Performs a raw call to an account on the current state of the VM.
    pub fn call_raw(
        &self,
        from: Address,
        to: Address,
        calldata: Bytes,
        value: U256,
    ) -> eyre::Result<
            RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
    > {
        let env = self.build_test_env(from, TxKind::Call(to), calldata, value);
        self.call_with_env(env)
    }

    /// Performs a raw call to an account on the current state of the VM.
    pub fn transact_raw(
        &mut self,
        from: Address,
        to: Address,
        calldata: Bytes,
        value: U256,
    ) -> eyre::Result<RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
        let env = self.build_test_env(from, TxKind::Call(to), calldata, value);
        self.transact_with_env(env)
    }

    /// Execute the transaction configured in `env.tx`.
    ///
    /// The state after the call is **not** persisted.
    #[instrument(name = "call", level = "debug", skip_all)]
    pub fn call_with_env(&self, mut env: EvmEnv<BlockT, TxT, HardforkT>) -> eyre::Result<
        RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
    > {
        let mut inspector = self.inspector().clone();
        let mut backend = CowBackend::new_borrowed(self.backend());
        let result_and_state = backend.inspect(&mut env, self.chain_context.clone(), &mut inspector)?;
        let result = convert_executed_result(env, inspector, result_and_state, backend.has_state_snapshot_failure(), backend.indeterminism_reasons())?;
        Ok(result)
    }

    /// Execute the transaction configured in `env.tx`.
    #[instrument(name = "transact", level = "debug", skip_all)]
    pub fn transact_with_env(&mut self, mut env: EvmEnv<BlockT, TxT, HardforkT>) -> eyre::Result<RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
        let mut inspector = self.inspector().clone();
        let result = self.backend.inspect(&mut env, self.chain_context.clone(), &mut inspector)?;
        let mut result =
            convert_executed_result(env, inspector, result, self.backend.has_state_snapshot_failure(), self.backend.indeterminism_reasons())?;
        self.commit(&mut result);
        Ok(result)
    }

    /// Commit the changeset to the database and adjust `self.inspector_config` values according to
    /// the executed call result.
    ///
    /// This should not be exposed to the user, as it should be called only by `transact*`.
    #[instrument(name = "commit", level = "debug", skip_all)]
    fn commit(&mut self, result: &mut RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>) {
        // Persist changes to db.
        self.backend_mut().commit(result.state_changeset.clone());

        // Persist cheatcode state.
        self.inspector_mut().cheatcodes = result.cheatcodes.take();
        if let Some(cheats) = self.inspector_mut().cheatcodes.as_mut() {
            cheats.ignored_traces.ignored.clear();

            // if tracing was paused but never unpaused, we should begin next frame with tracing
            // still paused
            if let Some(last_pause_call) = cheats.ignored_traces.last_pause_call.as_mut() {
                *last_pause_call = (0, 0);
            }
        }

        // Persist the changed environment.
        self.inspector_mut().set_env(result.env.clone());
    }

    /// Returns `true` if a test can be considered successful.
    ///
    /// This is the same as [`Self::is_success`], but will consume the `state_changeset` map to use
    /// internally when calling `failed()`.
    pub fn is_raw_call_mut_success(
        &self,
        address: Address,
        call_result: &mut RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
        should_fail: bool,
    ) -> bool {
        self.is_raw_call_success(
            address,
            Cow::Owned(std::mem::take(&mut call_result.state_changeset)),
            call_result,
            should_fail,
        )
    }

    /// Returns `true` if a test can be considered successful.
    ///
    /// This is the same as [`Self::is_success`], but intended for outcomes of [`Self::call_raw`].
    pub fn is_raw_call_success(
        &self,
        address: Address,
        state_changeset: Cow<'_, StateChangeset>,
        call_result: &RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
        should_fail: bool,
    ) -> bool {
        if call_result.has_state_snapshot_failure {
            // a failure occurred in a reverted snapshot, which is considered a failed test
            return should_fail;
        }
        self.is_success(address, call_result.reverted, state_changeset, should_fail)
    }

    /// Returns `true` if a test can be considered successful.
    ///
    /// If the call succeeded, we also have to check the global and local failure flags.
    ///
    /// These are set by the test contract itself when an assertion fails, using the internal `fail`
    /// function. The global flag is located in [`CHEATCODE_ADDRESS`] at slot [`GLOBAL_FAIL_SLOT`],
    /// and the local flag is located in the test contract at an unspecified slot.
    ///
    /// This behavior is inherited from Dapptools, where initially only a public
    /// `failed` variable was used to track test failures, and later, a global failure flag was
    /// introduced to track failures across multiple contracts in
    /// [ds-test#30](https://github.com/dapphub/ds-test/pull/30).
    ///
    /// The assumption is that the test runner calls `failed` on the test contract to determine if
    /// it failed. However, we want to avoid this as much as possible, as it is relatively
    /// expensive to set up an EVM call just for checking a single boolean flag.
    ///
    /// See:
    /// - Newer DSTest: <https://github.com/dapphub/ds-test/blob/e282159d5170298eb2455a6c05280ab5a73a4ef0/src/test.sol#L47-L63>
    /// - Older DSTest: <https://github.com/dapphub/ds-test/blob/9ca4ecd48862b40d7b0197b600713f64d337af12/src/test.sol#L38-L49>
    /// - forge-std: <https://github.com/foundry-rs/forge-std/blob/19891e6a0b5474b9ea6827ddb90bb9388f7acfc0/src/StdAssertions.sol#L38-L44>
    pub fn is_success(
        &self,
        address: Address,
        reverted: bool,
        state_changeset: Cow<'_, StateChangeset>,
        should_fail: bool,
    ) -> bool {
        let success = self.is_success_raw(address, reverted, state_changeset);
        should_fail ^ success
    }

    #[instrument(name = "is_success", level = "debug", skip_all)]
    fn is_success_raw(
        &self,
        address: Address,
        reverted: bool,
        state_changeset: Cow<'_, StateChangeset>,
    ) -> bool {
        // The call reverted.
        if reverted {
            return false;
        }

        // A failure occurred in a reverted snapshot, which is considered a failed test.
        if self.backend().has_state_snapshot_failure() {
            return false;
        }

        // Check the global failure slot.
        if let Some(acc) = state_changeset.get(&CHEATCODE_ADDRESS)
            && let Some(failed_slot) = acc.storage.get(&GLOBAL_FAIL_SLOT)
            && !failed_slot.present_value().is_zero()
        {
            return false;
        }
        if let Ok(failed_slot) = self.backend().storage_ref(CHEATCODE_ADDRESS, GLOBAL_FAIL_SLOT)
            && !failed_slot.is_zero()
        {
            return false;
        }

        true
    }

    /// Creates the environment to use when executing a transaction in a test context
    ///
    /// If using a backend with cheatcodes, `tx.gas_price` and `block.number` will be overwritten by
    /// the cheatcode state in between calls.
    fn build_test_env(&self, caller: Address, kind: TxKind, data: Bytes, value: U256) -> EvmEnv<BlockT, TxT, HardforkT> {
        let mut cfg = self.env.cfg.clone();
        cfg.spec = self.spec_id();

        let mut block = self.env.block.clone();
        // We always set the gas price to 0 so we can execute the transaction regardless
        // of network conditions - the actual gas price is kept in `self.block`
        // and is applied by the cheatcode handler if it is enabled
        block.set_basefee(0);
        block.set_gas_limit(self.gas_limit);

        let mut tx = self.env.tx.clone();
        tx.set_caller(caller);
        tx.set_kind(kind);
        tx.set_data(data);
        tx.set_value(value);
        // As above, we set the gas price to 0.
        tx.set_gas_price(0);
        tx.set_gas_priority_fee(None);
        tx.set_gas_limit(self.gas_limit);

        EvmEnv { cfg, block, tx }
    }

    pub fn call_sol_default<C: SolCall>(&self, to: Address, args: &C) -> C::Return
    where
        C::Return: Default,
    {
        self.call_sol(CALLER, to, args, U256::ZERO, None)
            .map(|c| c.decoded_result)
            .inspect_err(|e| warn!(target: "forge::test", "failed calling {:?}: {e}", C::SIGNATURE))
            .unwrap_or_default()
    }

}

/// Represents the context after an execution error occurred.
#[derive(Debug, thiserror::Error)]
#[error("execution reverted: {reason} (gas: {})", raw.gas_used)]
pub struct ExecutionErr<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> {
    /// The raw result of the call.
    pub raw: RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
    /// The revert reason.
    pub reason: String,
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> std::ops::Deref for ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    type Target = RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> std::ops::DerefMut for ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.raw
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EvmError<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> {
    /// Error which occurred during execution of a transaction.
    #[error(transparent)]
    Execution(#[from] Box<ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>>),
    /// Error which occurred during ABI encoding/decoding.
    #[error(transparent)]
    Abi(#[from] alloy_dyn_abi::Error),
    /// Error caused which occurred due to calling the `skip` cheatcode.
    #[error("{0}")]
    Skip(SkipReason),
    /// Any other error.
    #[error("{0}")]
    Eyre(
        #[from]
        #[source]
        eyre::Report,
    ),
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> From<ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> for EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    fn from(err: ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>) -> Self {
        Self::Execution(Box::new(err))
    }
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> From<alloy_sol_types::Error> for EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    fn from(err: alloy_sol_types::Error) -> Self {
        Self::Abi(err.into())
    }
}

/// The result of a deployment.
#[derive(Debug)]
pub struct DeployResult<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> {
    /// The raw result of the deployment.
    pub raw: RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
    /// The address of the deployed contract
    pub address: Address,
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> std::ops::Deref for DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    type Target = RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> std::ops::DerefMut for DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.raw
    }
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> From<DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> for RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    fn from(d: DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>) -> Self {
        d.raw
    }
}

/// The result of a raw call.
#[derive(Debug)]
pub struct RawCallResult<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
> {
    /// The status of the call
    pub exit_reason: Option<InstructionResult>,
    /// Whether the call reverted or not
    pub reverted: bool,
    /// Whether the call includes a snapshot failure
    ///
    /// This is tracked separately from revert because a snapshot failure can occur without a
    /// revert, since assert failures are stored in a global variable (ds-test legacy)
    pub has_state_snapshot_failure: bool,
    /// The raw result of the call.
    pub result: Bytes,
    /// The gas used for the call
    pub gas_used: u64,
    /// Refunded gas
    pub gas_refunded: u64,
    /// The initial gas stipend for the transaction
    pub stipend: u64,
    /// The logs emitted during the call
    pub logs: Vec<Log>,
    /// The labels assigned to addresses during the call
    pub labels: AddressHashMap<String>,
    /// The traces of the call
    pub traces: Option<SparsedTraceArena>,
    /// The line coverage info collected during the call
    pub line_coverage: Option<HitMaps>,
    /// The edge coverage info collected during the call
    pub edge_coverage: Option<Vec<u8>>,
    /// The changeset of the state.
    pub state_changeset: StateChangeset,
    /// The `revm::Env` after the call
    pub env: EvmEnv<BlockT, TxT, HardforkT>,
    /// The cheatcode states after execution
    pub cheatcodes: Option<Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>>,
    /// The raw output of the execution
    pub out: Option<Output>,
    pub reverter: Option<Address>,
    pub indeterminism_reasons: Option<IndeterminismReasons>
}

impl<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
> Default for RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    fn default() -> Self {
        Self {
            exit_reason: None,
            reverted: false,
            has_state_snapshot_failure: false,
            result: Bytes::new(),
            gas_used: 0,
            gas_refunded: 0,
            stipend: 0,
            logs: Vec::new(),
            labels: HashMap::default(),
            traces: None,
            line_coverage: None,
            edge_coverage: None,
            state_changeset: HashMap::default(),
            env: EvmEnv::default_with_spec_id(HardforkT::default()),
            cheatcodes: Default::default(),
            out: None,
            reverter: None,
            indeterminism_reasons: None,
        }
    }
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: 'static + ChainContextTr, EvmBuilderT: 'static + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: 'static + HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    /// Unpacks an EVM result.
    pub fn from_evm_result(r: Result<Self, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>>) -> eyre::Result<(Self, Option<String>)> {
        match r {
            Ok(r) => Ok((r, None)),
            Err(EvmError::Execution(e)) => Ok((e.raw, Some(e.reason))),
            Err(e) => Err(e.into()),
        }
    }

    /// Unpacks an execution result.
    pub fn from_execution_result(r: Result<Self, ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>>) -> (Self, Option<String>) {
        match r {
            Ok(r) => (r, None),
            Err(e) => (e.raw, Some(e.reason)),
        }
    }

    /// Converts the result of the call into an `EvmError`.
    pub fn into_evm_error(self, rd: Option<&RevertDecoder>) -> EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
        if let Some(reason) = SkipReason::decode(&self.result) {
            return EvmError::Skip(reason);
        }
        let reason = rd.unwrap_or_default().decode(&self.result, self.exit_reason);
        EvmError::Execution(Box::new(self.into_execution_error(reason)))
    }

    /// Converts the result of the call into an `ExecutionErr`.
    pub fn into_execution_error(self, reason: String) -> ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
        ExecutionErr { raw: self, reason }
    }

    /// Returns an `EvmError` if the call failed, otherwise returns `self`.
    pub fn into_result(self, rd: Option<&RevertDecoder>) -> Result<Self, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
        if let Some(reason) = self.exit_reason
            && reason.is_ok()
        {
            Ok(self)
        } else {
            Err(self.into_evm_error(rd))
        }
    }

    /// Decodes the result of the call with the given function.
    pub fn into_decoded_result(
        mut self,
        func: &Function,
        rd: Option<&RevertDecoder>,
    ) -> Result<CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
        self = self.into_result(rd)?;
        let mut result = func.abi_decode_output(&self.result)?;
        let decoded_result = if result.len() == 1 {
            result.pop().unwrap()
        } else {
            // combine results into a tuple
            DynSolValue::Tuple(result)
        };
        Ok(CallResult { raw: self, decoded_result })
    }

    /// Update provided history map with edge coverage info collected during this call.
    /// Uses AFL binning algo <https://github.com/h0mbre/Lucid/blob/3026e7323c52b30b3cf12563954ac1eaa9c6981e/src/coverage.rs#L57-L85>
    pub fn merge_edge_coverage(&mut self, history_map: &mut [u8]) -> (bool, bool) {
        let mut new_coverage = false;
        let mut is_edge = false;
        if let Some(x) = &mut self.edge_coverage {
            // Iterate over the current map and the history map together and update
            // the history map, if we discover some new coverage, report true
            for (curr, hist) in std::iter::zip(x, history_map) {
                // If we got a hitcount of at least 1
                if *curr > 0 {
                    // Convert hitcount into bucket count
                    let bucket = match *curr {
                        0 => 0,
                        1 => 1,
                        2 => 2,
                        3 => 4,
                        4..=7 => 8,
                        8..=15 => 16,
                        16..=31 => 32,
                        32..=127 => 64,
                        128..=255 => 128,
                    };

                    // If the old record for this edge pair is lower, update
                    if *hist < bucket {
                        if *hist == 0 {
                            // Counts as an edge the first time we see it, otherwise it's a feature.
                            is_edge = true;
                        }
                        *hist = bucket;
                        new_coverage = true;
                    }

                    // Zero out the current map for next iteration.
                    *curr = 0;
                }
            }
        }
        (new_coverage, is_edge)
    }
}

/// The result of a call.
pub struct CallResult<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait, T = DynSolValue> {
    /// The raw result of the call.
    pub raw: RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
    /// The decoded result of the call.
    pub decoded_result: T,
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> std::ops::Deref for CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    type Target = RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, ChainContextT: ChainContextTr, EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>, HaltReasonT: HaltReasonTr, HardforkT: HardforkTr, TransactionErrorT: TransactionErrorTrait> std::ops::DerefMut for CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.raw
    }
}

/// Converts the data aggregated in the `inspector` and `call` to a `RawCallResult`
fn convert_executed_result<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr + TryInto<HaltReason>,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
>(
    env: EvmEnv<BlockT, TxT, HardforkT>,
    inspector: InspectorStack<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>,
    ResultAndState { result, state: state_changeset }: ResultAndState<HaltReasonT>,
    has_state_snapshot_failure: bool,
    indeterminism_reasons: Option<IndeterminismReasons>
) -> eyre::Result<RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>> {
    let (exit_reason, gas_refunded, gas_used, out, exec_logs) = match result {
        ExecutionResult::Success { reason, gas_used, gas_refunded, output, logs, .. } => {
            (reason.into(), gas_refunded, gas_used, Some(output), logs)
        }
        ExecutionResult::Revert { gas_used, output } => {
            // Need to fetch the unused gas
            (InstructionResult::Revert, 0_u64, gas_used, Some(Output::Call(output)), vec![])
        }
        ExecutionResult::Halt { reason, gas_used } => {
            let reason: HaltReason = reason.clone().try_into().map_err(|_error| {
                eyre::eyre!("Halt reason cannot be converted to `HaltReason`: {reason:?}")
            })?;

            (reason.into(), 0_u64, gas_used, None, vec![])
        }
    };
    let gas = revm::interpreter::gas::calculate_initial_tx_gas(
        env.cfg.spec.into(),
        &env.tx.input(),
        env.tx.kind().is_create(),
        env.tx.access_list().map_or(0, Iterator::count).try_into()?,
        0,
        0,
    );

    let result = match &out {
        Some(Output::Call(data)) => data.clone(),
        _ => Bytes::new(),
    };

    let InspectorData {
        mut logs,
        labels,
        traces,
        line_coverage,
        edge_coverage,
        cheatcodes,
        reverter,
    } = inspector.collect()?;

    if logs.is_empty() {
        logs = exec_logs;
    }

    Ok(RawCallResult {
        exit_reason: Some(exit_reason),
        reverted: !matches!(exit_reason, return_ok!()),
        has_state_snapshot_failure,
        result,
        gas_used,
        gas_refunded,
        stipend: gas.initial_gas,
        logs,
        labels,
        traces,
        line_coverage,
        edge_coverage,
        state_changeset,
        env,
        cheatcodes,
        out,
        reverter,
        indeterminism_reasons
    })
}

/// Timer for a fuzz test.
pub struct FuzzTestTimer {
    /// Inner fuzz test timer - (test start time, test duration).
    inner: Option<(Instant, Duration)>,
}

impl FuzzTestTimer {
    pub fn new(timeout: Option<u32>) -> Self {
        Self { inner: timeout.map(|timeout| (Instant::now(), Duration::from_secs(timeout.into()))) }
    }

    /// Whether the current fuzz test timed out and should be stopped.
    pub fn is_timed_out(&self) -> bool {
        self.inner.is_some_and(|(start, duration)| start.elapsed() > duration)
    }
}
