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
use alloy_primitives::{
    map::{AddressHashMap, HashMap},
    Address, Bytes, Log, TxKind, U256,
};
use alloy_sol_types::{sol, SolCall};
use derive_where::derive_where;
use foundry_evm_core::{
    backend::{Backend, BackendError, BackendResult, CheatcodeBackend, CowBackend},
    constants::{
        CALLER, CHEATCODE_ADDRESS, DEFAULT_CREATE2_DEPLOYER, DEFAULT_CREATE2_DEPLOYER_CODE,
    },
    decode::RevertDecoder,
    evm_context::EvmBuilderTrait,
    utils::StateChangeset,
};
use foundry_evm_coverage::HitMaps;
use foundry_evm_traces::SparsedTraceArena;
use revm::{
    bytecode::Bytecode,
    context::result::{ExecutionResult, HaltReasonTr, ResultAndState},
    context_interface::result::Output,
    database::{DatabaseCommit, DatabaseRef},
    interpreter::{return_ok, InstructionResult},
};

use crate::inspectors::{Cheatcodes, InspectorData, InspectorStack};

mod builder;
pub use builder::ExecutorBuilder;
use foundry_evm_core::evm_context::{
    BlockEnvTr, ChainContextTr, EvmEnv, HardforkTr, TransactionEnvTr,
};
// Leaving this intentionally removed as it was flagged as unused
use foundry_evm_core::{backend::IndeterminismReasons, decode::SkipReason};

pub mod fuzz;
pub use fuzz::FuzzedExecutor;

pub mod invariant;
pub mod stack_trace;

pub use invariant::InvariantExecutor;

sol! {
    interface ITest {
        function setUp() external;
        function failed() external view returns (bool);
    }
}

/// A type that can execute calls
///
/// The executor can be configured with various `revm::Inspector`s, like
/// `Cheatcodes`.
///
/// There are two ways of executing calls:
/// - `committing`: any state changes made during the call are recorded and are
///   persisting
/// - `raw`: state changes only exist for the duration of the call and are
///   discarded afterwards, in other words: the state of the underlying database
///   remains unchanged.
#[derive_where(Clone, Debug; BlockT, HardforkT, TxT)]
pub struct Executor<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    ChainContextT: ChainContextTr,
> {
    /// The underlying `revm::Database` that contains the EVM storage.
    // Note: We do not store an EVM here, since we are really
    // only interested in the database. REVM's `EVM` is a thin
    // wrapper around spawning a new EVM on every call anyway,
    // so the performance difference should be negligible.
    pub backend: Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    /// The EVM environment.
    pub env: EvmEnv<BlockT, TxT, HardforkT>,
    /// The Revm inspector stack.
    pub inspector: InspectorStack<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    chain_context: ChainContextT,
    /// The gas limit for calls and deployments. This is different from the gas
    /// limit imposed by the passed in environment, as those limits are used
    /// by the EVM for certain opcodes like `gaslimit`.
    gas_limit: u64,
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
    > Executor<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>
{
    #[inline]
    pub fn new(
        mut backend: Backend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
        env: EvmEnv<BlockT, TxT, HardforkT>,
        chain_context: ChainContextT,
        inspector: InspectorStack<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
        gas_limit: u64,
    ) -> Self {
        // Need to create a non-empty contract on the cheatcodes address so
        // `extcodesize` checks does not fail
        backend.insert_account_info(
            CHEATCODE_ADDRESS,
            revm::state::AccountInfo {
                code: Some(Bytecode::new_raw(Bytes::from_static(&[0]))),
                ..Default::default()
            },
        );

        Executor {
            backend,
            env,
            inspector,
            chain_context,
            gas_limit,
        }
    }

    /// Returns the spec id of the executor
    pub fn spec_id(&self) -> HardforkT {
        self.env.cfg.spec
    }

    /// Set the balance of an account.
    pub fn set_balance(&mut self, address: Address, amount: U256) -> BackendResult<&mut Self> {
        trace!(?address, ?amount, "setting account balance");
        let mut account = self.backend.basic_ref(address)?.unwrap_or_default();
        account.balance = amount;

        self.backend.insert_account_info(address, account);
        Ok(self)
    }

    /// Gets the balance of an account
    pub fn get_balance(&self, address: Address) -> BackendResult<U256> {
        Ok(self
            .backend
            .basic_ref(address)?
            .map(|acc| acc.balance)
            .unwrap_or_default())
    }

    /// Set the nonce of an account.
    pub fn set_nonce(&mut self, address: Address, nonce: u64) -> BackendResult<&mut Self> {
        let mut account = self.backend.basic_ref(address)?.unwrap_or_default();
        account.nonce = nonce;
        self.backend.insert_account_info(address, account);
        Ok(self)
    }

    /// Gets the nonce of an account
    pub fn get_nonce(&self, address: Address) -> BackendResult<u64> {
        Ok(self
            .backend
            .basic_ref(address)?
            .map(|acc| acc.nonce)
            .unwrap_or_default())
    }

    /// Returns true if account has no code.
    pub fn is_empty_code(&self, address: Address) -> BackendResult<bool> {
        Ok(self
            .backend
            .basic_ref(address)?
            .as_ref()
            .is_none_or(revm::state::AccountInfo::is_empty_code_hash))
    }

    #[inline]
    pub fn set_tracing(&mut self, tracing: bool) -> &mut Self {
        self.inspector.tracing(tracing);
        self
    }

    #[inline]
    pub fn set_gas_limit(&mut self, gas_limit: u64) -> &mut Self {
        self.gas_limit = gas_limit;
        self
    }

    /// Commit the changeset to the database and adjust `self.inspector_config`
    /// values according to the executed call result
    fn commit(
        &mut self,
        result: &mut RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) {
        // Persist changes to db.
        self.backend.commit(result.state_changeset.clone());

        // Persist cheatcode state.
        let cheatcodes = result.cheatcodes.take();
        self.inspector.cheatcodes = cheatcodes;

        // Persist the changed environment.
        self.inspector.set_env(result.env.clone());
    }

    /// Creates the environment to use when executing a transaction in a test
    /// context
    ///
    /// If using a backend with cheatcodes, `tx.gas_price` and `block.number`
    /// will be overwritten by the cheatcode state inbetween calls.
    fn build_test_env(
        &self,
        caller: Address,
        transact_to: TxKind,
        data: Bytes,
        value: U256,
    ) -> EvmEnv<BlockT, TxT, HardforkT> {
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
        tx.set_transact_to(transact_to);
        tx.set_input(data);
        tx.set_value(value);
        // As above, we set the gas price to 0.
        tx.set_gas_price(0);
        tx.set_gas_priority_fee(None);
        tx.set_gas_limit(self.gas_limit);

        EvmEnv { cfg, block, tx }
    }

    /// Whether when re-executing the calls the same results are guaranteed.
    pub fn safe_to_re_execute(&self) -> bool {
        self.backend.safe_to_re_execute()
    }

    pub fn indeterminism_reasons(&self) -> Option<IndeterminismReasons> {
        self.backend.indeterminism_reasons()
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: 'static + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: 'static + HaltReasonTr + Into<InstructionResult>,
        HardforkT: HardforkTr,
        ChainContextT: 'static + ChainContextTr,
    > Executor<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>
{
    /// Performs a call to an account on the current state of the VM.
    ///
    /// The state after the call is not persisted.
    #[allow(clippy::type_complexity)]
    pub fn call(
        &self,
        from: Address,
        to: Address,
        func: &Function,
        args: &[DynSolValue],
        value: U256,
        rd: Option<&RevertDecoder>,
    ) -> Result<
        CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
        EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    > {
        let calldata = Bytes::from(func.abi_encode_input(args)?);
        let (result, _cow_backend) = self.call_raw(from, to, calldata, value)?;
        result.into_decoded_result(func, rd)
    }

    /// Performs a call to an account on the current state of the VM.
    ///
    /// The state after the call is persisted.
    #[allow(clippy::type_complexity)]
    pub fn call_committing(
        &mut self,
        from: Address,
        to: Address,
        func: &Function,
        args: &[DynSolValue],
        value: U256,
        rd: Option<&RevertDecoder>,
    ) -> Result<
        CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
        EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    > {
        let calldata = Bytes::from(func.abi_encode_input(args)?);
        let result = self.call_raw_committing(from, to, calldata, value)?;
        result.into_decoded_result(func, rd)
    }

    /// Performs a call to an account on the current state of the VM.
    ///
    /// The state after the call is not persisted.
    #[allow(clippy::type_complexity)]
    pub fn call_sol<C: SolCall>(
        &self,
        from: Address,
        to: Address,
        args: &C,
        value: U256,
        rd: Option<&RevertDecoder>,
    ) -> Result<
        CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, C::Return>,
        EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    > {
        let calldata = Bytes::from(args.abi_encode());
        let (mut raw, _cow_backend) = self.call_raw(from, to, calldata, value)?;
        raw = raw.into_result(rd)?;
        Ok(CallResult {
            decoded_result: C::abi_decode_returns(&raw.result, false)?,
            raw,
        })
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

    /// Deploys a contract and commits the new state to the underlying database.
    ///
    /// Executes a CREATE transaction with the contract `code` and persistent
    /// database state modifications.
    #[allow(clippy::type_complexity)]
    pub fn deploy(
        &mut self,
        from: Address,
        code: Bytes,
        value: U256,
        rd: Option<&RevertDecoder>,
    ) -> Result<
        DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
        EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    > {
        let env = self.build_test_env(from, TxKind::Create, code, value);
        self.deploy_with_env(env, rd)
    }

    /// Creates the default CREATE2 Contract Deployer for local tests and
    /// scripts.
    pub fn deploy_create2_deployer(&mut self) -> eyre::Result<()> {
        trace!("deploying local create2 deployer");
        let create2_deployer_account = self
            .backend
            .basic_ref(DEFAULT_CREATE2_DEPLOYER)?
            .ok_or_else(|| BackendError::MissingAccount(DEFAULT_CREATE2_DEPLOYER))?;

        // if the deployer is not currently deployed, deploy the default one
        if create2_deployer_account
            .code
            .as_ref()
            .is_none_or(revm::bytecode::Bytecode::is_empty)
        {
            let creator = "0x3fAB184622Dc19b6109349B94811493BF2a45362"
                .parse()
                .unwrap();

            // Probably 0, but just in case.
            let initial_balance = self.get_balance(creator)?;

            self.set_balance(creator, U256::MAX)?;
            let res = self.deploy(
                creator,
                DEFAULT_CREATE2_DEPLOYER_CODE.into(),
                U256::ZERO,
                None,
            )?;
            trace!(create2=?res.address, "deployed local create2 deployer");

            self.set_balance(creator, initial_balance)?;
        }
        Ok(())
    }

    /// Deploys a contract using the given `env` and commits the new state to
    /// the underlying database.
    ///
    /// # Panics
    ///
    /// Panics if `env.tx.transact_to` is not `TxKind::Create(_)`.
    #[allow(clippy::type_complexity)]
    pub fn deploy_with_env(
        &mut self,
        env: EvmEnv<BlockT, TxT, HardforkT>,
        rd: Option<&RevertDecoder>,
    ) -> Result<
        DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
        EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    > {
        assert!(
            matches!(env.tx.kind(), TxKind::Create),
            "Expected create transaction, got {:?}",
            env.tx.kind()
        );
        trace!(sender=%env.tx.caller(), "deploying contract");

        let mut result = self.call_raw_with_env(env)?;
        self.commit(&mut result);
        result = result.into_result(rd)?;
        let Some(Output::Create(_, Some(address))) = result.out else {
            panic!("Deployment succeeded, but no address was returned: {result:#?}");
        };

        // also mark this library as persistent, this will ensure that the state of the
        // library is persistent across fork swaps in forking mode
        self.backend.add_persistent_account(address);

        debug!(%address, "deployed contract");

        Ok(DeployResult {
            raw: result,
            address,
        })
    }

    /// Executes the test function call
    #[allow(clippy::type_complexity)]
    pub fn execute_test(
        &mut self,
        from: Address,
        test_contract: Address,
        func: &Function,
        args: &[DynSolValue],
        value: U256,
        rd: Option<&RevertDecoder>,
    ) -> Result<
        CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
        EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    > {
        let calldata = Bytes::from(func.abi_encode_input(args)?);

        // execute the call
        let env = self.build_test_env(from, TxKind::Call(test_contract), calldata, value);
        let result = self.call_raw_with_env(env)?;
        result.into_decoded_result(func, rd)
    }

    /// Returns `true` if a test can be considered successful.
    ///
    /// This is the same as [`Self::is_success`], but will consume the
    /// `state_changeset` map to use internally when calling `failed()`.
    pub fn is_raw_call_mut_success(
        &self,
        address: Address,
        call_result: &mut RawCallResult<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
        >,
        should_fail: bool,
    ) -> bool {
        self.is_raw_call_success(
            address,
            Cow::Owned(std::mem::take(&mut call_result.state_changeset)),
            call_result,
            should_fail,
        )
    }

    /// This is the same as [`Self::is_success`] but intended for outcomes of
    /// [`Self::call_raw`] used in fuzzing and invariant testing.
    ///
    /// ## Background
    ///
    /// Executing and failure checking [`Executor::ensure_success`] are two
    /// steps, for ds-test legacy reasons failures can be stored in a global
    /// variables and needs to be called via a solidity call
    /// `failed()(bool)`.
    ///
    /// For fuzz tests we’re using the `CowBackend` which is a Cow of the
    /// executor’s backend which lazily clones the backend when it’s mutated
    /// via cheatcodes like `snapshot`. Snapshots make it even more
    /// complicated because now we also need to keep track of that global
    /// variable when we revert to a snapshot (because it is stored in state).
    /// Now, the problem is that the `CowBackend` is dropped after every
    /// call, so we need to keep track of the snapshot failure in the
    /// [`RawCallResult`] instead.
    pub fn is_raw_call_success(
        &self,
        address: Address,
        state_changeset: Cow<'_, StateChangeset>,
        call_result: &RawCallResult<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
        >,
        should_fail: bool,
    ) -> bool {
        if call_result.has_state_snapshot_failure {
            // a failure occurred in a reverted snapshot, which is considered a failed test
            return should_fail;
        }
        self.is_success(address, call_result.reverted, state_changeset, should_fail)
    }

    /// Check if a call to a test contract was successful.
    ///
    /// This function checks both the VM status of the call, `DSTest`'s `failed`
    /// status and the `globalFailed` flag which is stored in `failed`
    /// inside the `CHEATCODE_ADDRESS` contract.
    ///
    /// `DSTest` will not revert inside its `assertEq`-like functions which
    /// allows to test multiple assertions in 1 test function while also
    /// preserving logs.
    ///
    /// If an `assert` is violated, the contract's `failed` variable is set to
    /// true, and the `globalFailure` flag inside the `CHEATCODE_ADDRESS` is
    /// also set to true, this way, failing asserts from any contract are
    /// tracked as well.
    ///
    /// In order to check whether a test failed, we therefore need to evaluate
    /// the contract's `failed` variable and the `globalFailure` flag, which
    /// happens by calling `contract.failed()`.
    pub fn is_success(
        &self,
        address: Address,
        reverted: bool,
        state_changeset: Cow<'_, StateChangeset>,
        should_fail: bool,
    ) -> bool {
        self.ensure_success(address, reverted, state_changeset, should_fail)
            .unwrap_or_default()
    }

    /// Calls the `setUp()` function on a contract.
    ///
    /// This will commit any state changes to the underlying database.
    ///
    /// Ayn changes made during the setup call to env's block environment are
    /// persistent, for example `vm.chainId()` will change the
    /// `block.chainId` for all subsequent test calls.
    #[allow(clippy::type_complexity)]
    pub fn setup(
        &mut self,
        from: Option<Address>,
        to: Address,
        rd: Option<&RevertDecoder>,
    ) -> Result<
        RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
        EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    > {
        trace!(?from, ?to, "setting up contract");

        let from = from.unwrap_or(CALLER);
        self.backend.set_test_contract(to).set_caller(from);
        let calldata = Bytes::from_static(&ITest::setUpCall::SELECTOR);
        let mut res = self.call_raw_committing(from, to, calldata, U256::ZERO)?;
        res = res.into_result(rd)?;

        // record any changes made to the block's environment during setup
        self.env.block = res.env.block.clone();
        // and also the chainid, which can be set manually
        self.env.cfg.chain_id = res.env.cfg.chain_id;

        let success =
            self.is_raw_call_success(to, Cow::Borrowed(&res.state_changeset), &res, false);
        if !success {
            return Err(res
                .into_execution_error("execution error".to_string())
                .into());
        }

        Ok(res)
    }

    fn ensure_success(
        &self,
        address: Address,
        reverted: bool,
        state_changeset: Cow<'_, StateChangeset>,
        should_fail: bool,
    ) -> Result<bool, BackendError> {
        if self.backend.has_state_snapshot_failure() {
            // a failure occurred in a reverted snapshot, which is considered a failed test
            return Ok(should_fail);
        }

        let mut success = !reverted;
        if success {
            // Construct a new bare-bones backend to evaluate success.
            let mut backend = self.backend.clone_empty();

            // We only clone the test contract and cheatcode accounts,
            // that's all we need to evaluate success.
            for addr in [address, CHEATCODE_ADDRESS] {
                let acc = self.backend.basic_ref(addr)?.unwrap_or_default();
                backend.insert_account_info(addr, acc);
            }

            // If this test failed any asserts, then this changeset will contain changes
            // `false -> true` for the contract's `failed` variable and the `globalFailure`
            // flag in the state of the cheatcode address,
            // which are both read when we call `"failed()(bool)"` in the next step.
            backend.commit(state_changeset.into_owned());

            // Check if a DSTest assertion failed
            let executor = Executor::new(
                backend,
                self.env.clone(),
                self.chain_context.clone(),
                self.inspector.clone(),
                self.gas_limit,
            );
            let call = executor.call_sol(CALLER, address, &ITest::failedCall {}, U256::ZERO, None);
            if let Ok(CallResult {
                raw: _,
                decoded_result: ITest::failedReturn { _0: failed },
            }) = call
            {
                debug!(failed, "DSTest::failed()");
                success = !failed;
            }
        }

        let result = should_fail ^ success;
        debug!(should_fail, success, result);
        Ok(result)
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr + Into<InstructionResult>,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
    > Executor<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>
{
    /// Performs a raw call to an account on the current state of the VM.
    ///
    /// The state after the call is persisted.
    pub fn call_raw_committing(
        &mut self,
        from: Address,
        to: Address,
        calldata: Bytes,
        value: U256,
    ) -> eyre::Result<RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>>
    {
        let env = self.build_test_env(from, TxKind::Call(to), calldata, value);
        let mut result = self.call_raw_with_env(env)?;
        self.commit(&mut result);
        Ok(result)
    }

    /// Performs a raw call to an account on the current state of the VM.
    ///
    /// Any state modifications made by the call are not committed.
    ///
    /// This intended for fuzz calls, which try to minimize [Backend] clones by
    /// using a Cow of the underlying [Backend] so it only gets cloned when
    /// cheatcodes that require mutable access are used. The method returns the
    /// `CowBackend`, as changes to `CowBackend` are not persisted in the
    /// executor's backend.
    #[allow(clippy::type_complexity)]
    pub fn call_raw(
        &self,
        from: Address,
        to: Address,
        calldata: Bytes,
        value: U256,
    ) -> eyre::Result<(
        RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
        CowBackend<'_, BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    )> {
        let mut inspector = self.inspector.clone();
        // Build VM
        let mut env = self.build_test_env(from, TxKind::Call(to), calldata, value);
        let mut db = CowBackend::new(&self.backend);
        let result = db.inspect(&mut env, &mut inspector, self.chain_context.clone())?;

        // Persist the state snapshot failure recorded on the fuzz backend wrapper.
        let has_state_snapshot_failure = db.has_state_snapshot_failure();
        Ok((
            convert_executed_result(env, inspector, result, has_state_snapshot_failure)?,
            db,
        ))
    }

    /// Execute the transaction configured in `env.tx`
    pub fn call_raw_with_env(
        &mut self,
        mut env: EvmEnv<BlockT, TxT, HardforkT>,
    ) -> eyre::Result<RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>>
    {
        // execute the call
        let mut inspector = self.inspector.clone();
        let result = self
            .backend
            .inspect(&mut env, self.chain_context.clone(), &mut inspector)?;
        convert_executed_result(
            env,
            inspector,
            result,
            self.backend.has_state_snapshot_failure(),
        )
    }

    /// Execute the transaction configured in `env.tx` and commit the changes
    pub fn commit_tx_with_env(
        &mut self,
        env: EvmEnv<BlockT, TxT, HardforkT>,
    ) -> eyre::Result<RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>>
    {
        let mut result = self.call_raw_with_env(env)?;
        self.commit(&mut result);
        Ok(result)
    }
}

/// Represents the context after an execution error occurred.
#[derive_where(Debug; BlockT, TxT, HardforkT)]
#[derive(thiserror::Error)]
#[error("execution reverted: {reason} (gas: {})", raw.gas_used)]
pub struct ExecutionErr<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
> {
    /// The raw result of the call.
    pub raw: RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    /// The revert reason.
    pub reason: String,
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    > std::ops::Deref
    for ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    type Target = RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    > std::ops::DerefMut
    for ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.raw
    }
}

#[derive_where(Debug; BlockT, TxT, HardforkT)]
#[derive(thiserror::Error)]
pub enum EvmError<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: 'static + ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
> {
    /// Error which occurred during execution of a transaction
    #[error(transparent)]
    Execution(
        #[from] Box<ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>>,
    ),
    /// Error which occurred during ABI encoding/decoding
    #[error(transparent)]
    AbiError(#[from] alloy_dyn_abi::Error),
    /// Error caused which occurred due to calling the `skip` cheatcode.
    #[error("{0}")]
    Skip(SkipReason),
    /// Any other error.
    #[error(transparent)]
    Eyre(#[from] eyre::Error),
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    > From<ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>>
    for EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    fn from(
        err: ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Self {
        EvmError::Execution(Box::new(err))
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    > From<alloy_sol_types::Error>
    for EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    fn from(err: alloy_sol_types::Error) -> Self {
        EvmError::AbiError(err.into())
    }
}

/// The result of a deployment.
#[derive(Debug)]
pub struct DeployResult<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
> {
    /// The raw result of the deployment.
    pub raw: RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    /// The address of the deployed contract
    pub address: Address,
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    > std::ops::Deref
    for DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    type Target = RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    > std::ops::DerefMut
    for DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.raw
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    > From<DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>>
    for RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    fn from(
        d: DeployResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Self {
        d.raw
    }
}

/// The result of a raw call.
#[derive_where(Debug; BlockT, TxT, HardforkT)]
pub struct RawCallResult<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
> {
    /// The status of the call
    pub exit_reason: InstructionResult,
    /// Whether the call reverted or not
    pub reverted: bool,
    /// Whether the call includes a state snapshot failure
    ///
    /// This is tracked separately from revert because a state snapshot failure
    /// can occur without a revert, since assert failures are stored in a
    /// global variable (ds-test legacy)
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
    /// The coverage info collected during the call
    pub coverage: Option<HitMaps>,
    /// The changeset of the state.
    pub state_changeset: StateChangeset,
    /// The env after the call
    pub env: EvmEnv<BlockT, TxT, HardforkT>,
    /// The cheatcode states after execution
    pub cheatcodes:
        Option<Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>>,
    /// The raw output of the execution
    pub out: Option<Output>,
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    > Default for RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    fn default() -> Self {
        Self {
            exit_reason: InstructionResult::Continue,
            reverted: false,
            has_state_snapshot_failure: false,
            result: Bytes::new(),
            gas_used: 0,
            gas_refunded: 0,
            stipend: 0,
            logs: Vec::new(),
            labels: AddressHashMap::default(),
            traces: None,
            coverage: None,
            state_changeset: HashMap::default(),
            env: EvmEnv::new_with_spec_id(HardforkT::default()),
            cheatcodes: Option::default(),
            out: None,
        }
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: 'static + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: 'static + HaltReasonTr,
        HardforkT: HardforkTr,
    > RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    /// Unpacks an EVM result.
    pub fn from_evm_result(
        r: Result<Self, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>>,
    ) -> eyre::Result<(Self, Option<String>)> {
        match r {
            Ok(r) => Ok((r, None)),
            Err(EvmError::Execution(e)) => Ok((e.raw, Some(e.reason))),
            Err(e) => Err(e.into()),
        }
    }

    /// Unpacks an execution result.
    pub fn from_execution_result(
        r: Result<
            Self,
            ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
        >,
    ) -> (Self, Option<String>) {
        match r {
            Ok(r) => (r, None),
            Err(e) => (e.raw, Some(e.reason)),
        }
    }

    /// Converts the result of the call into an `EvmError`.
    pub fn into_evm_error(
        self,
        rd: Option<&RevertDecoder>,
    ) -> EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT> {
        if let Some(reason) = SkipReason::decode(&self.result) {
            return EvmError::Skip(reason);
        }
        let reason = rd
            .unwrap_or_default()
            .decode(&self.result, Some(self.exit_reason));
        EvmError::Execution(Box::new(self.into_execution_error(reason)))
    }

    /// Converts the result of the call into an `ExecutionErr`.
    pub fn into_execution_error(
        self,
        reason: String,
    ) -> ExecutionErr<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT> {
        ExecutionErr { raw: self, reason }
    }

    /// Returns an `EvmError` if the call failed, otherwise returns `self`.
    pub fn into_result(
        self,
        rd: Option<&RevertDecoder>,
    ) -> Result<Self, EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>>
    {
        if self.exit_reason.is_ok() {
            Ok(self)
        } else {
            Err(self.into_evm_error(rd))
        }
    }

    /// Decodes the result of the call with the given function.
    #[allow(clippy::type_complexity)]
    pub fn into_decoded_result(
        mut self,
        func: &Function,
        rd: Option<&RevertDecoder>,
    ) -> Result<
        CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
        EvmError<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    > {
        self = self.into_result(rd)?;
        let mut result = func.abi_decode_output(&self.result, false)?;
        let decoded_result = if result.len() == 1 {
            result.pop().unwrap()
        } else {
            // combine results into a tuple
            DynSolValue::Tuple(result)
        };
        Ok(CallResult {
            raw: self,
            decoded_result,
        })
    }
}

/// The result of a call.
pub struct CallResult<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    DecodedResultT = DynSolValue,
> {
    /// The raw result of the call.
    pub raw: RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    /// The decoded result of the call.
    pub decoded_result: DecodedResultT,
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    > std::ops::Deref
    for CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    type Target = RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    > std::ops::DerefMut
    for CallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.raw
    }
}

/// Converts the data aggregated in the `inspector` and `call` to a
/// `RawCallResult`
fn convert_executed_result<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr + Into<InstructionResult>,
    HardforkT: HardforkTr,
    ChainContextT: ChainContextTr,
>(
    env: EvmEnv<BlockT, TxT, HardforkT>,
    inspector: InspectorStack<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    result: ResultAndState<HaltReasonT>,
    has_state_snapshot_failure: bool,
) -> eyre::Result<RawCallResult<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>> {
    let ResultAndState {
        result: exec_result,
        state: state_changeset,
    } = result;
    let (exit_reason, gas_refunded, gas_used, out) = match exec_result {
        ExecutionResult::Success {
            reason,
            gas_used,
            gas_refunded,
            output,
            ..
        } => (reason.into(), gas_refunded, gas_used, Some(output)),
        ExecutionResult::Revert { gas_used, output } => {
            // Need to fetch the unused gas
            (
                InstructionResult::Revert,
                0_u64,
                gas_used,
                Some(Output::Call(output)),
            )
        }
        ExecutionResult::Halt { reason, gas_used } => (reason.into(), 0_u64, gas_used, None),
    };

    let gas = revm::interpreter::gas::calculate_initial_tx_gas(
        env.cfg.spec.into(),
        env.tx.input(),
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
        logs,
        labels,
        traces,
        coverage,
        cheatcodes,
    } = inspector.collect();

    Ok(RawCallResult {
        exit_reason,
        reverted: !matches!(exit_reason, return_ok!()),
        has_state_snapshot_failure,
        result,
        gas_used,
        gas_refunded,
        stipend: gas.initial_gas,
        logs,
        labels,
        traces,
        coverage,
        state_changeset,
        env,
        cheatcodes,
        out,
    })
}

/// Timer for a fuzz test.
pub struct FuzzTestTimer {
    /// Inner fuzz test timer - (test start time, test duration).
    inner: Option<(Instant, Duration)>,
}

impl FuzzTestTimer {
    pub fn new(timeout: Option<u32>) -> Self {
        Self {
            inner: timeout.map(|timeout| (Instant::now(), Duration::from_secs(timeout.into()))),
        }
    }

    /// Whether the current fuzz test timed out and should be stopped.
    pub fn is_timed_out(&self) -> bool {
        self.inner
            .is_some_and(|(start, duration)| start.elapsed() > duration)
    }
}
