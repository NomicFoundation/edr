//! Cheatcode EVM [Inspector].

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    fmt::Debug,
    fs::File,
    io::BufReader,
    ops::Range,
    path::PathBuf,
    sync::Arc,
};

use alloy_primitives::{Address, Bytes, Log, TxKind, B256, U256};
use alloy_rpc_types::request::TransactionRequest;
use alloy_sol_types::{SolInterface, SolValue};
use foundry_evm_core::{
    abi::Vm::stopExpectSafeMemoryCall,
    backend::{CheatcodeBackend, RevertDiagnostic},
    constants::{CHEATCODE_ADDRESS, HARDHAT_CONSOLE_ADDRESS},
    evm_context::{BlockEnvTr, ChainContextTr, HardforkTr, TransactionEnvTr},
};
use itertools::Itertools;
use revm::{
    self,
    bytecode::opcode,
    context::{BlockEnv, CfgEnv, Context as EvmContext, JournalTr},
    interpreter::{
        interpreter_types::{Jumps, MemoryTr},
        CallInputs, CallOutcome, CallScheme, CreateInputs, CreateOutcome, Gas, Host,
        InstructionResult, Interpreter, InterpreterAction, InterpreterResult,
    },
    Inspector, Journal,
};
use rustc_hash::FxHashMap;
use serde_json::Value;
use upstream_foundry_cheatcodes_spec::Vm as UpstreamVM;

use crate::{
    evm::{
        mapping::{self, MappingSlots},
        mock::{MockCallDataContext, MockCallReturnData},
        prank::Prank,
        DealRecord, RecordAccess,
    },
    test::expect::{
        self, ExpectedCallData, ExpectedCallTracker, ExpectedCallType, ExpectedEmit,
        ExpectedRevert, ExpectedRevertKind,
    },
    CheatsConfig, CheatsCtxt, DynCheatcode, Error, Result, Vm,
    Vm::AccountAccess,
};

macro_rules! try_or_continue {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(_) => return,
        }
    };
}

/// Contains additional, test specific resources that should be kept for the
/// duration of the test
#[derive(Debug, Default)]
pub struct Context {
    /// Buffered readers for files opened for reading (path => `BufReader`
    /// mapping)
    pub opened_read_files: HashMap<PathBuf, BufReader<File>>,
}

/// Every time we clone `Context`, we want it to be empty
impl Clone for Context {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl Context {
    /// Clears the context.
    #[inline]
    pub fn clear(&mut self) {
        self.opened_read_files.clear();
    }
}

/// Helps collecting transactions from different forks.
#[derive(Clone, Debug, Default)]
pub struct BroadcastableTransaction {
    /// The optional RPC URL.
    pub rpc: Option<String>,
    /// The transaction to broadcast.
    pub transaction: TransactionRequest,
}

/// List of transactions that can be broadcasted.
pub type BroadcastableTransactions = VecDeque<BroadcastableTransaction>;

/// An EVM inspector that handles calls to various cheatcodes, each with their
/// own behavior.
///
/// Cheatcodes can be called by contracts during execution to modify the VM
/// environment, such as mocking addresses, signatures and altering call
/// reverts.
///
/// Executing cheatcodes can be very powerful. Most cheatcodes are limited to
/// evm internals, but there are also cheatcodes like `ffi` which can execute
/// arbitrary commands or `writeFile` and `readFile` which can manipulate files
/// of the filesystem. Therefore, several restrictions are implemented for these
/// cheatcodes:
/// - `ffi`, and file cheatcodes are _always_ opt-in (via foundry config) and
///   never enabled by default: all respective cheatcode handlers implement the
///   appropriate checks
/// - File cheatcodes require explicit permissions which paths are allowed for
///   which operation, see `Config.fs_permission`
/// - Only permitted accounts are allowed to execute cheatcodes in forking mode,
///   this ensures no contract deployed on the live network is able to execute
///   cheatcodes by simply calling the cheatcode address: by default, the
///   caller, test contract and newly deployed contracts are allowed to execute
///   cheatcodes
#[derive(Clone, Debug, Default)]
// Need bounds for `Unpin` for `Arc`
pub struct Cheatcodes<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr> {
    /// The block environment
    ///
    /// Used in the cheatcode handler to overwrite the block environment
    /// separately from the execution block environment.
    // TODO this is fine for OP, but need to be made generic for other chains
    pub block: Option<BlockEnv>,

    /// The gas price
    ///
    /// Used in the cheatcode handler to overwrite the gas price separately from
    /// the gas price in the execution environment.
    pub gas_price: Option<u128>,

    /// Address labels
    pub labels: HashMap<Address, String>,

    /// Prank information
    pub prank: Option<Prank>,

    /// Expected revert information
    pub expected_revert: Option<ExpectedRevert>,

    /// Additional diagnostic for reverts
    pub fork_revert_diagnostic: Option<RevertDiagnostic>,

    /// Recorded storage reads and writes
    pub accesses: Option<RecordAccess>,

    /// Recorded account accesses (calls, creates) organized by relative call
    /// depth, where the topmost vector corresponds to accesses at the depth
    /// at which account access recording began. Each vector in the matrix
    /// represents a list of accesses at a specific call depth. Once that
    /// call context has ended, the last vector is removed from the matrix and
    /// merged into the previous vector.
    pub recorded_account_diffs_stack: Option<Vec<Vec<AccountAccess>>>,

    /// Recorded logs
    pub recorded_logs: Option<Vec<crate::Vm::Log>>,

    /// Cache of the amount of gas used in previous call.
    /// This is used by the `lastCallGas` cheatcode.
    pub last_call_gas: Option<crate::Vm::Gas>,

    /// Mocked calls
    // **Note**: inner must a BTreeMap because of special `Ord` impl for `MockCallDataContext`
    pub mocked_calls: HashMap<Address, BTreeMap<MockCallDataContext, MockCallReturnData>>,

    /// Expected calls
    pub expected_calls: ExpectedCallTracker,
    /// Expected emits
    pub expected_emits: VecDeque<ExpectedEmit>,

    /// Map of context depths to memory offset ranges that may be written to
    /// within the call depth.
    pub allowed_mem_writes: FxHashMap<u64, Vec<Range<u64>>>,

    /// Additional, user configurable context this Inspector has access to when
    /// inspecting a call
    pub config: Arc<CheatsConfig<BlockT, TxT, HardforkT>>,

    /// Test-scoped context holding data that needs to be reset every test run
    pub context: Context,

    /// Whether to commit FS changes such as file creations, writes and deletes.
    /// Used to prevent duplicate changes file executing non-committing calls.
    pub fs_commit: bool,

    /// Serialized JSON values.
    // **Note**: both must a BTreeMap to ensure the order of the keys is deterministic.
    pub serialized_jsons: BTreeMap<String, BTreeMap<String, Value>>,

    /// All recorded ETH `deal`s.
    pub eth_deals: Vec<DealRecord>,

    /// Holds the stored gas info for when we pause gas metering. It is an
    /// `Option<Option<..>>` because the `call` callback in an `Inspector`
    /// doesn't get access to the `revm::Interpreter` which holds the
    /// `revm::Gas` struct that we need to copy. So we convert it to a
    /// `Some(None)` in `apply_cheatcode`, and once we have the interpreter,
    /// we copy the gas struct. Then each time there is an execution of an
    /// operation, we reset the gas.
    pub gas_metering: Option<Option<Gas>>,

    /// Holds stored gas info for when we pause gas metering, and we're
    /// entering/inside CREATE / CREATE2 frames. This is needed to make gas
    /// meter pausing work correctly when paused and creating new contracts.
    pub gas_metering_create: Option<Option<Gas>>,

    /// Mapping slots.
    pub mapping_slots: Option<HashMap<Address, MappingSlots>>,

    /// The current program counter.
    pub pc: usize,

    /// Deprecated cheatcodes mapped to the reason. Used to report warnings on
    /// test results.
    pub deprecated: HashMap<&'static str, Option<&'static str>>,
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr>
    Cheatcodes<BlockT, TxT, HardforkT>
{
    /// Creates a new `Cheatcodes` with the given settings.
    #[inline]
    pub fn new(config: Arc<CheatsConfig<BlockT, TxT, HardforkT>>) -> Self {
        let labels = config.labels.clone();
        Self {
            config,
            fs_commit: true,
            labels,
            ..Default::default()
        }
    }

    fn apply_cheatcode<
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, HardforkT, ChainContextT>,
    >(
        &mut self,
        ecx: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        call: &CallInputs,
    ) -> Result {
        // decode the cheatcode call
        let decoded = Vm::VmCalls::abi_decode(&call.input, false).map_err(|e| {
            if let alloy_sol_types::Error::UnknownSelector { name: _, selector } = e {
                let message = if let Some(unsupported_cheatcode) =
                    find_upstream_cheatcode_signature(selector)
                {
                    format!("cheatcode '{unsupported_cheatcode}' is not supported",)
                } else {
                    format!("unknown cheatcode with selector '{selector}'")
                };
                return alloy_sol_types::Error::Other(std::borrow::Cow::Owned(message));
            }
            e
        })?;
        let caller = call.caller;

        // ensure the caller is allowed to execute cheatcodes,
        // but only if the backend is in forking mode
        ecx.journaled_state
            .db()
            .ensure_cheatcode_access_forking_mode(&caller)?;

        apply_dispatch(
            &decoded,
            &mut CheatsCtxt {
                state: self,
                ecx,
                caller,
            },
        )
    }

    /// Determines the address of the contract and marks it as allowed
    /// Returns the address of the contract created
    ///
    /// There may be cheatcodes in the constructor of the new contract, in order
    /// to allow them automatically we need to determine the new address
    #[allow(clippy::unused_self)]
    fn allow_cheatcodes_on_create<
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, HardforkT, ChainContextT>,
    >(
        &self,
        ecx: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        inputs: &CreateInputs,
    ) -> Address {
        let old_nonce = ecx
            .journaled_state
            .inner
            .state
            .get(&inputs.caller)
            .map(|acc| acc.info.nonce)
            .unwrap_or_default();
        let created_address = inputs.created_address(old_nonce);

        if ecx.journaled_state.depth > 1
            && !ecx
                .journaled_state
                .database
                .has_cheatcode_access(&inputs.caller)
        {
            // we only grant cheat code access for new contracts if the caller also has
            // cheatcode access and the new contract is created in top most call
            return created_address;
        }

        ecx.journaled_state
            .database
            .allow_cheatcode_access(created_address);

        created_address
    }

    /// Called when there was a revert.
    ///
    /// Cleanup any previously applied cheatcodes that altered the state in such
    /// a way that revm's revert would run into issues.
    pub fn on_revert<
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, HardforkT, ChainContextT>,
    >(
        &mut self,
        ecx: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
    ) {
        trace!(deals=?self.eth_deals.len(), "rolling back deals");

        // Delay revert clean up until expected revert is handled, if set.
        if self.expected_revert.is_some() {
            return;
        }

        // we only want to apply cleanup top level
        if ecx.journaled_state.depth() > 0 {
            return;
        }

        // Roll back all previously applied deals
        // This will prevent overflow issues in revm's
        // [`JournaledState::journal_revert`] routine which rolls back any
        // transfers.
        while let Some(record) = self.eth_deals.pop() {
            if let Some(acc) = ecx.journaled_state.inner.state.get_mut(&record.address) {
                acc.info.balance = record.old_balance;
            }
        }
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, HardforkT, ChainContextT>,
    >
    Inspector<
        EvmContext<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, Journal<DatabaseT>, ChainContextT>,
    > for Cheatcodes<BlockT, TxT, HardforkT>
{
    #[inline]
    fn initialize_interp(
        &mut self,
        _: &mut Interpreter,
        ecx: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
    ) {
        // When the first interpreter is initialized we've circumvented the balance and
        // gas checks, so we apply our actual block data with the correct fees
        // and all.
        if let Some(block) = self.block.take() {
            ecx.block = block.into();
        }
        if let Some(gas_price) = self.gas_price.take() {
            ecx.tx.set_gas_price(gas_price);
        }
    }

    fn step(
        &mut self,
        interpreter: &mut Interpreter,
        ecx: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
    ) {
        self.pc = interpreter.bytecode.pc();

        // reset gas if gas metering is turned off
        match self.gas_metering {
            Some(None) => {
                // need to store gas metering
                self.gas_metering = Some(Some(interpreter.control.gas));
            }
            Some(Some(gas)) => {
                match interpreter.bytecode.opcode() {
                    opcode::CREATE | opcode::CREATE2 => {
                        // set we're about to enter CREATE frame to meter its gas on first opcode
                        // inside it
                        self.gas_metering_create = Some(None);
                    }
                    opcode::STOP | opcode::RETURN | opcode::SELFDESTRUCT | opcode::REVERT => {
                        // If we are ending current execution frame, we want to just fully reset gas
                        // otherwise weird things with returning gas from a call happen
                        // ref: https://github.com/bluealloy/revm/blob/2cb991091d32330cfe085320891737186947ce5a/crates/revm/src/evm_impl.rs#L190
                        //
                        // It would be nice if we had access to the interpreter in `call_end`, as we
                        // could just do this there instead.
                        match self.gas_metering_create {
                            None | Some(None) => {
                                interpreter.control.gas = Gas::new(0);
                            }
                            Some(Some(gas)) => {
                                // If this was CREATE frame, set correct gas limit. This is needed
                                // because CREATE opcodes deduct additional gas for code storage,
                                // and deducted amount is compared to gas limit. If we set this to
                                // 0, the CREATE would fail with out of gas.
                                //
                                // If we however set gas limit to the limit of outer frame, it would
                                // cause a panic after erasing gas cost post-create. Reason for this
                                // is pre-create REVM records `gas_limit - (gas_limit / 64)` as gas
                                // used, and erases costs by `remaining` gas post-create.
                                // gas used ref: https://github.com/bluealloy/revm/blob/2cb991091d32330cfe085320891737186947ce5a/crates/revm/src/instructions/host.rs#L254-L258
                                // post-create erase ref: https://github.com/bluealloy/revm/blob/2cb991091d32330cfe085320891737186947ce5a/crates/revm/src/instructions/host.rs#L279
                                interpreter.control.gas = Gas::new(gas.limit());

                                // reset CREATE gas metering because we're about to exit its frame
                                self.gas_metering_create = None;
                            }
                        }
                    }
                    _ => {
                        // if just starting with CREATE opcodes, record its inner frame gas
                        if let Some(None) = self.gas_metering_create {
                            self.gas_metering_create = Some(Some(interpreter.control.gas));
                        }

                        // dont monitor gas changes, keep it constant
                        interpreter.control.gas = gas;
                    }
                }
            }
            _ => {}
        }

        // Record writes and reads if `record` has been called
        if let Some(storage_accesses) = &mut self.accesses {
            match interpreter.bytecode.opcode() {
                opcode::SLOAD => {
                    let key = try_or_continue!(interpreter.stack.peek(0));
                    storage_accesses
                        .reads
                        .entry(interpreter.input.target_address)
                        .or_default()
                        .push(key);
                }
                opcode::SSTORE => {
                    let key = try_or_continue!(interpreter.stack.peek(0));

                    // An SSTORE does an SLOAD internally
                    storage_accesses
                        .reads
                        .entry(interpreter.input.target_address)
                        .or_default()
                        .push(key);
                    storage_accesses
                        .writes
                        .entry(interpreter.input.target_address)
                        .or_default()
                        .push(key);
                }
                _ => (),
            }
        }

        // Record account access via SELFDESTRUCT if `recordAccountAccesses` has been
        // called
        if let Some(account_accesses) = &mut self.recorded_account_diffs_stack {
            if interpreter.bytecode.opcode() == opcode::SELFDESTRUCT {
                let target = try_or_continue!(interpreter.stack.peek(0));
                // load balance of this account
                let value = ecx
                    .balance(interpreter.input.target_address)
                    .map_or(U256::ZERO, |b| b.data);
                let account = Address::from_word(B256::from(target));
                // get previous balance and initialized status of the target account
                let (initialized, old_balance) = ecx
                    .journaled_state
                    .load_account(account)
                    .map(|account| (account.info.exists(), account.info.balance))
                    .unwrap_or_default();

                // register access for the target account
                let access = crate::Vm::AccountAccess {
                    chainInfo: crate::Vm::ChainInfo {
                        forkId: ecx
                            .journaled_state
                            .database
                            .active_fork_id()
                            .unwrap_or_default(),
                        chainId: U256::from(ecx.cfg.chain_id),
                    },
                    accessor: interpreter.input.target_address,
                    account,
                    kind: crate::Vm::AccountAccessKind::SelfDestruct,
                    initialized,
                    oldBalance: old_balance,
                    newBalance: old_balance + value,
                    value,
                    data: Bytes::new(),
                    reverted: false,
                    deployedCode: Bytes::new(),
                    storageAccesses: vec![],
                    depth: ecx.journaled_state.depth() as u64,
                };
                // Ensure that we're not selfdestructing a context recording was initiated on
                if let Some(last) = account_accesses.last_mut() {
                    last.push(access);
                }
            }
        }

        // Record granular ordered storage accesses if `startStateDiffRecording` has
        // been called
        if let Some(recorded_account_diffs_stack) = &mut self.recorded_account_diffs_stack {
            match interpreter.bytecode.opcode() {
                opcode::SLOAD => {
                    let key = try_or_continue!(interpreter.stack.peek(0));
                    let address = interpreter.input.target_address;

                    // Try to include present value for informational purposes, otherwise assume
                    // it's not set (zero value)
                    let mut present_value = U256::ZERO;
                    // Try to load the account and the slot's present value
                    if ecx.journaled_state.load_account(address).is_ok() {
                        if let Some(previous) = ecx.sload(address, key) {
                            present_value = previous.data;
                        }
                    }
                    let access = crate::Vm::StorageAccess {
                        account: interpreter.input.target_address,
                        slot: key.into(),
                        isWrite: false,
                        previousValue: present_value.into(),
                        newValue: present_value.into(),
                        reverted: false,
                    };
                    let curr_depth = ecx.journaled_state.depth() as u64;
                    append_storage_access(recorded_account_diffs_stack, access, curr_depth);
                }
                opcode::SSTORE => {
                    let key = try_or_continue!(interpreter.stack.peek(0));
                    let value = try_or_continue!(interpreter.stack.peek(1));
                    let address = interpreter.input.target_address;
                    // Try to load the account and the slot's previous value, otherwise, assume it's
                    // not set (zero value)
                    let mut previous_value = U256::ZERO;
                    if ecx.journaled_state.load_account(address).is_ok() {
                        if let Some(previous) = ecx.sload(address, key) {
                            previous_value = previous.data;
                        }
                    }

                    let access = crate::Vm::StorageAccess {
                        account: address,
                        slot: key.into(),
                        isWrite: true,
                        previousValue: previous_value.into(),
                        newValue: value.into(),
                        reverted: false,
                    };
                    let curr_depth = ecx.journaled_state.depth() as u64;
                    append_storage_access(recorded_account_diffs_stack, access, curr_depth);
                }
                // Record account accesses via the EXT family of opcodes
                opcode::EXTCODECOPY
                | opcode::EXTCODESIZE
                | opcode::EXTCODEHASH
                | opcode::BALANCE => {
                    let kind = match interpreter.bytecode.opcode() {
                        opcode::EXTCODECOPY => crate::Vm::AccountAccessKind::Extcodecopy,
                        opcode::EXTCODESIZE => crate::Vm::AccountAccessKind::Extcodesize,
                        opcode::EXTCODEHASH => crate::Vm::AccountAccessKind::Extcodehash,
                        opcode::BALANCE => crate::Vm::AccountAccessKind::Balance,
                        _ => unreachable!(),
                    };
                    let address =
                        Address::from_word(B256::from(try_or_continue!(interpreter.stack.peek(0))));
                    let (initialized, balance) = ecx
                        .journaled_state
                        .load_account(address)
                        .map(|account| (account.info.exists(), account.info.balance))
                        .unwrap_or_default();
                    let curr_depth = ecx.journaled_state.depth() as u64;
                    let account_access = crate::Vm::AccountAccess {
                        chainInfo: crate::Vm::ChainInfo {
                            forkId: ecx
                                .journaled_state
                                .database
                                .active_fork_id()
                                .unwrap_or_default(),
                            chainId: U256::from(ecx.cfg.chain_id),
                        },
                        accessor: interpreter.input.target_address,
                        account: address,
                        kind,
                        initialized,
                        oldBalance: balance,
                        newBalance: balance,
                        value: U256::ZERO,
                        data: Bytes::new(),
                        reverted: false,
                        deployedCode: Bytes::new(),
                        storageAccesses: vec![],
                        depth: curr_depth,
                    };
                    // Record the EXT* call as an account access at the current depth
                    // (future storage accesses will be recorded in a new "Resume" context)
                    if let Some(last) = recorded_account_diffs_stack.last_mut() {
                        last.push(account_access);
                    } else {
                        recorded_account_diffs_stack.push(vec![account_access]);
                    }
                }
                _ => (),
            }
        }

        // If the allowed memory writes cheatcode is active at this context depth, check
        // to see if the current opcode can either mutate directly or expand
        // memory. If the opcode at the current program counter is a match,
        // check if the modified memory lies within the allowed ranges. If not,
        // revert and fail the test.
        let depth = ecx.journaled_state.depth() as u64;
        if let Some(ranges) = self.allowed_mem_writes.get(&depth) {
            // The `mem_opcode_match` macro is used to match the current opcode against a
            // list of opcodes that can mutate memory (either directly or
            // expansion via reading). If the opcode is a match, the memory
            // offsets that are being written to are checked to be within the
            // allowed ranges. If not, the test is failed and the transaction is
            // reverted. For all opcodes that can mutate memory aside from MSTORE,
            // MSTORE8, and MLOAD, the size and destination offset are on the stack, and
            // the macro expands all of these cases. For MSTORE, MSTORE8, and MLOAD, the
            // size of the memory write is implicit, so these cases are hard-coded.
            macro_rules! mem_opcode_match {
                ($(($opcode:ident, $offset_depth:expr, $size_depth:expr, $writes:expr)),* $(,)?) => {
                    match interpreter.bytecode.opcode() {
                        ////////////////////////////////////////////////////////////////
                        //    OPERATIONS THAT CAN EXPAND/MUTATE MEMORY BY WRITING     //
                        ////////////////////////////////////////////////////////////////

                        opcode::MSTORE => {
                            // The offset of the mstore operation is at the top of the stack.
                            let offset = try_or_continue!(interpreter.stack.peek(0)).saturating_to::<u64>();

                            // If none of the allowed ranges contain [offset, offset + 32), memory has been
                            // unexpectedly mutated.
                            if !ranges.iter().any(|range| {
                                range.contains(&offset) && range.contains(&(offset + 31))
                            }) {
                                // SPECIAL CASE: When the compiler attempts to store the selector for
                                // `stopExpectSafeMemory`, this is allowed. It will do so at the current free memory
                                // pointer, which could have been updated to the exclusive upper bound during
                                // execution.
                                let value = try_or_continue!(interpreter.stack.peek(1)).to_be_bytes::<32>();
                                let selector = stopExpectSafeMemoryCall {}.cheatcode().func.selector_bytes;
                                if value[0..edr_defaults::SELECTOR_LEN] == selector {
                                    return
                                }

                                disallowed_mem_write(offset, 32, interpreter, ranges);
                                return
                            }
                        }
                        opcode::MSTORE8 => {
                            // The offset of the mstore8 operation is at the top of the stack.
                            let offset = try_or_continue!(interpreter.stack.peek(0)).saturating_to::<u64>();

                            // If none of the allowed ranges contain the offset, memory has been
                            // unexpectedly mutated.
                            if !ranges.iter().any(|range| range.contains(&offset)) {
                                disallowed_mem_write(offset, 1, interpreter, ranges);
                                return
                            }
                        }

                        ////////////////////////////////////////////////////////////////
                        //        OPERATIONS THAT CAN EXPAND MEMORY BY READING        //
                        ////////////////////////////////////////////////////////////////

                        opcode::MLOAD => {
                            // The offset of the mload operation is at the top of the stack
                            let offset = try_or_continue!(interpreter.stack.peek(0)).saturating_to::<u64>();

                            // If the offset being loaded is >= than the memory size, the
                            // memory is being expanded. If none of the allowed ranges contain
                            // [offset, offset + 32), memory has been unexpectedly mutated.
                            if offset >= interpreter.memory.size() as u64 && !ranges.iter().any(|range| {
                                range.contains(&offset) && range.contains(&(offset + 31))
                            }) {
                                disallowed_mem_write(offset, 32, interpreter, ranges);
                                return
                            }
                        }

                        ////////////////////////////////////////////////////////////////
                        //          OPERATIONS WITH OFFSET AND SIZE ON STACK          //
                        ////////////////////////////////////////////////////////////////

                        opcode::CALL => {
                            // The destination offset of the operation is the fifth element on the stack.
                            let dest_offset = try_or_continue!(interpreter.stack.peek(5)).saturating_to::<u64>();

                            // The size of the data that will be copied is the sixth element on the stack.
                            let size = try_or_continue!(interpreter.stack.peek(6)).saturating_to::<u64>();

                            // If none of the allowed ranges contain [dest_offset, dest_offset + size),
                            // memory outside of the expected ranges has been touched. If the opcode
                            // only reads from memory, this is okay as long as the memory is not expanded.
                            let fail_cond = !ranges.iter().any(|range| {
                                range.contains(&dest_offset) &&
                                    range.contains(&(dest_offset + size.saturating_sub(1)))
                            });

                            // If the failure condition is met, set the output buffer to a revert string
                            // that gives information about the allowed ranges and revert.
                            if fail_cond {
                                // SPECIAL CASE: When a call to `stopExpectSafeMemory` is performed, this is allowed.
                                // It allocated calldata at the current free memory pointer, and will attempt to read
                                // from this memory region to perform the call.
                                let to = Address::from_word(try_or_continue!(interpreter.stack.peek(1)).to_be_bytes::<32>().into());
                                if to == CHEATCODE_ADDRESS {
                                    let args_offset = try_or_continue!(interpreter.stack.peek(3)).saturating_to::<usize>();
                                    let args_size = try_or_continue!(interpreter.stack.peek(4)).saturating_to::<usize>();
                                    let selector = stopExpectSafeMemoryCall {}.cheatcode().func.selector_bytes;
                                    let memory_word = interpreter.memory.slice_len(args_offset, args_size);
                                    if memory_word[0..edr_defaults::SELECTOR_LEN] == selector {
                                        return
                                    }
                                }

                                disallowed_mem_write(dest_offset, size, interpreter, ranges);
                                return
                            }
                        }

                        $(opcode::$opcode => {
                            // The destination offset of the operation.
                            let dest_offset = try_or_continue!(interpreter.stack.peek($offset_depth)).saturating_to::<u64>();

                            // The size of the data that will be copied.
                            let size = try_or_continue!(interpreter.stack.peek($size_depth)).saturating_to::<u64>();

                            // If none of the allowed ranges contain [dest_offset, dest_offset + size),
                            // memory outside of the expected ranges has been touched. If the opcode
                            // only reads from memory, this is okay as long as the memory is not expanded.
                            let fail_cond = !ranges.iter().any(|range| {
                                    range.contains(&dest_offset) &&
                                        range.contains(&(dest_offset + size.saturating_sub(1)))
                                }) && ($writes ||
                                    [dest_offset, (dest_offset + size).saturating_sub(1)].into_iter().any(|offset| {
                                        offset >= interpreter.memory.size() as u64
                                    })
                                );

                            // If the failure condition is met, set the output buffer to a revert string
                            // that gives information about the allowed ranges and revert.
                            if fail_cond {
                                disallowed_mem_write(dest_offset, size, interpreter, ranges);
                                return
                            }
                        })*
                        _ => ()
                    }
                }
            }

            // Check if the current opcode can write to memory, and if so, check if the
            // memory being written to is registered as safe to modify.
            mem_opcode_match!(
                (CALLDATACOPY, 0, 2, true),
                (CODECOPY, 0, 2, true),
                (RETURNDATACOPY, 0, 2, true),
                (EXTCODECOPY, 1, 3, true),
                (CALLCODE, 5, 6, true),
                (STATICCALL, 4, 5, true),
                (DELEGATECALL, 4, 5, true),
                (KECCAK256, 0, 1, false),
                (LOG0, 0, 1, false),
                (LOG1, 0, 1, false),
                (LOG2, 0, 1, false),
                (LOG3, 0, 1, false),
                (LOG4, 0, 1, false),
                (CREATE, 1, 2, false),
                (CREATE2, 1, 2, false),
                (RETURN, 0, 1, false),
                (REVERT, 0, 1, false),
            );
        }

        // Record writes with sstore (and sha3) if `StartMappingRecording` has been
        // called
        if let Some(mapping_slots) = &mut self.mapping_slots {
            mapping::step(mapping_slots, interpreter);
        }
    }

    fn log(
        &mut self,
        _interpreter: &mut Interpreter,
        _context: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        log: Log,
    ) {
        if !self.expected_emits.is_empty() {
            expect::handle_expect_emit(self, &log);
        }

        // Stores this log if `recordLogs` has been called
        if let Some(storage_recorded_logs) = &mut self.recorded_logs {
            storage_recorded_logs.push(Vm::Log {
                topics: log.data.topics().to_vec(),
                data: log.data.data.clone(),
                emitter: log.address,
            });
        }
    }

    fn call(
        &mut self,
        ecx: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        call: &mut CallInputs,
    ) -> Option<CallOutcome> {
        let gas = Gas::new(call.gas_limit);

        // At the root call to test function or script `run()`/`setUp()` functions, we
        // are decreasing sender nonce to ensure that it matches on-chain nonce
        // once we start broadcasting.
        if ecx.journaled_state.depth == 0 {
            let sender = ecx.tx.caller();
            if sender != edr_defaults::SOLIDITY_TESTS_SENDER {
                let account = match super::evm::journaled_account(ecx, sender) {
                    Ok(account) => account,
                    Err(err) => {
                        return Some(CallOutcome {
                            result: InterpreterResult {
                                result: InstructionResult::Revert,
                                output: err.abi_encode().into(),
                                gas,
                            },
                            memory_offset: call.return_memory_offset.clone(),
                        })
                    }
                };
                let prev = account.info.nonce;
                account.info.nonce = prev.saturating_sub(1);

                debug!(target: "cheatcodes", %sender, nonce=account.info.nonce, prev, "corrected nonce");
            }
        }

        if call.target_address == CHEATCODE_ADDRESS {
            return match self.apply_cheatcode(ecx, call) {
                Ok(retdata) => Some(CallOutcome {
                    result: InterpreterResult {
                        result: InstructionResult::Return,
                        output: retdata.into(),
                        gas,
                    },
                    memory_offset: call.return_memory_offset.clone(),
                }),
                Err(err) => Some(CallOutcome {
                    result: InterpreterResult {
                        result: InstructionResult::Revert,
                        output: err.abi_encode().into(),
                        gas,
                    },
                    memory_offset: call.return_memory_offset.clone(),
                }),
            };
        }

        if call.target_address == HARDHAT_CONSOLE_ADDRESS {
            return None;
        }

        // Handle expected calls

        // Grab the different calldatas expected.
        if let Some(expected_calls_for_target) = self.expected_calls.get_mut(&(call.target_address))
        {
            // Match every partial/full calldata
            for (calldata, (expected, actual_count)) in expected_calls_for_target {
                // Increment actual times seen if...
                // The calldata is at most, as big as this call's input, and
                if calldata.len() <= call.input.len() &&
                    // Both calldata match, taking the length of the assumed smaller one (which will have at least the selector), and
                    *calldata == call.input[..calldata.len()] &&
                    // The value matches, if provided
                    expected
                        .value.is_none_or(|value| Some(value) == call.transfer_value()) &&
                    // The gas matches, if provided
                    expected.gas.is_none_or(|gas| gas == call.gas_limit) &&
                    // The minimum gas matches, if provided
                    expected.min_gas.is_none_or(|min_gas| min_gas <= call.gas_limit)
                {
                    *actual_count += 1;
                }
            }
        }

        // Handle mocked calls
        if let Some(mocks) = self.mocked_calls.get(&call.target_address) {
            let ctx = MockCallDataContext {
                calldata: call.input.clone(),
                value: call.transfer_value(),
            };
            if let Some(return_data) = mocks.get(&ctx).or_else(|| {
                mocks
                    .iter()
                    .find(|(mock, _)| {
                        call.input.get(..mock.calldata.len()) == Some(&mock.calldata[..])
                            && mock
                                .value
                                .is_none_or(|value| Some(value) == call.transfer_value())
                    })
                    .map(|(_, v)| v)
            }) {
                return Some(CallOutcome {
                    result: InterpreterResult {
                        result: return_data.ret_type,
                        output: return_data.data.clone(),
                        gas,
                    },
                    memory_offset: call.return_memory_offset.clone(),
                });
            }
        }

        let curr_depth = ecx.journaled_state.depth() as u64;

        // Apply our prank
        if let Some(prank) = &self.prank {
            if curr_depth >= prank.depth && call.caller == prank.prank_caller {
                let mut prank_applied = false;

                // At the target depth we set `msg.sender`
                if curr_depth == prank.depth {
                    call.caller = prank.new_caller;
                    prank_applied = true;
                }

                // At the target depth, or deeper, we set `tx.origin`
                if let Some(new_origin) = prank.new_origin {
                    ecx.tx.set_caller(new_origin);
                    prank_applied = true;
                }

                // If prank applied for first time, then update
                if prank_applied {
                    if let Some(applied_prank) = prank.first_time_applied() {
                        self.prank = Some(applied_prank);
                    }
                }
            }
        }

        // Record called accounts if `startStateDiffRecording` has been called
        if let Some(recorded_account_diffs_stack) = &mut self.recorded_account_diffs_stack {
            // Determine if account is "initialized," ie, it has a non-zero balance, a
            // non-zero nonce, a non-zero KECCAK_EMPTY codehash, or non-empty
            // code
            let (initialized, old_balance) = ecx
                .journaled_state
                .load_account(call.target_address)
                .map(|account| (account.info.exists(), account.info.balance))
                .unwrap_or_default();
            let kind = match call.scheme {
                CallScheme::Call | CallScheme::ExtCall => crate::Vm::AccountAccessKind::Call,
                CallScheme::CallCode => crate::Vm::AccountAccessKind::CallCode,
                CallScheme::DelegateCall | CallScheme::ExtDelegateCall => {
                    crate::Vm::AccountAccessKind::DelegateCall
                }
                CallScheme::StaticCall | CallScheme::ExtStaticCall => {
                    crate::Vm::AccountAccessKind::StaticCall
                }
            };
            // Record this call by pushing it to a new pending vector; all subsequent calls
            // at that depth will be pushed to the same vector. When the call
            // ends, the RecordedAccountAccess (and all subsequent
            // RecordedAccountAccesses) will be updated with the revert status
            // of this call, since the EVM does not mark accounts as "warm" if
            // the call from which they were accessed is reverted
            recorded_account_diffs_stack.push(vec![AccountAccess {
                chainInfo: crate::Vm::ChainInfo {
                    forkId: ecx
                        .journaled_state
                        .database
                        .active_fork_id()
                        .unwrap_or_default(),
                    chainId: U256::from(ecx.cfg.chain_id),
                },
                accessor: call.caller,
                account: call.bytecode_address,
                kind,
                initialized,
                oldBalance: old_balance,
                newBalance: U256::ZERO, // updated on call_end
                value: call.call_value(),
                data: call.input.clone(),
                reverted: false,
                deployedCode: Bytes::new(),
                storageAccesses: vec![], // updated on step
                depth: curr_depth,
            }]);
        }

        None
    }

    fn call_end(
        &mut self,
        ecx: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        call: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        let cheatcode_call = call.target_address == CHEATCODE_ADDRESS
            || call.target_address == HARDHAT_CONSOLE_ADDRESS;

        let curr_depth = ecx.journaled_state.depth() as u64;

        // Clean up pranks/broadcasts if it's not a cheatcode call end. We shouldn't do
        // it for cheatcode calls because they are not appplied for cheatcodes in the
        // `call` hook. This should be placed before the revert handling,
        // because we might exit early there
        if !cheatcode_call {
            // Clean up pranks
            if let Some(prank) = &self.prank {
                if curr_depth == prank.depth {
                    ecx.tx.set_caller(prank.prank_origin);

                    // Clean single-call prank once we have returned to the original depth
                    if prank.single_call {
                        let _ = self.prank.take();
                    }
                }
            }
        }

        // Handle expected reverts
        if let Some(expected_revert) = &self.expected_revert {
            if curr_depth <= expected_revert.depth {
                let needs_processing: bool = match expected_revert.kind {
                    ExpectedRevertKind::Default => !cheatcode_call,
                    // `pending_processing` == true means that we're in the `call_end` hook for
                    // `vm.expectCheatcodeRevert` and shouldn't expect revert here
                    ExpectedRevertKind::Cheatcode { pending_processing } => {
                        cheatcode_call && !pending_processing
                    }
                };

                if needs_processing {
                    let expected_revert = std::mem::take(&mut self.expected_revert).unwrap();
                    return match expect::handle_expect_revert(
                        false,
                        expected_revert.reason.as_deref(),
                        outcome.result.result,
                        outcome.result.output.clone(),
                    ) {
                        Err(error) => {
                            trace!(expected=?expected_revert, ?error, status=?outcome.result.result, "Expected revert mismatch");
                            outcome.result.result = InstructionResult::Revert;
                            outcome.result.output = error.abi_encode().into();
                        }
                        Ok((_, retdata)) => {
                            outcome.result.result = InstructionResult::Return;
                            outcome.result.output = retdata;
                        }
                    };
                }

                // Flip `pending_processing` flag for cheatcode revert expectations, marking
                // that we've exited the `expectCheatcodeRevert` call scope
                if let ExpectedRevertKind::Cheatcode { pending_processing } =
                    &mut self.expected_revert.as_mut().unwrap().kind
                {
                    if *pending_processing {
                        *pending_processing = false;
                    }
                }
            }
        }

        // Exit early for calls to cheatcodes as other logic is not relevant for
        // cheatcode invocations
        if cheatcode_call {
            return;
        }

        // Record the gas usage of the call, this allows the `lastCallGas` cheatcode to
        // retrieve the gas usage of the last call.
        let gas = outcome.result.gas;
        self.last_call_gas = Some(crate::Vm::Gas {
            gasLimit: gas.limit(),
            gasTotalUsed: gas.spent(),
            gasMemoryUsed: 0,
            gasRefunded: gas.refunded(),
            gasRemaining: gas.remaining(),
        });

        // If `startStateDiffRecording` has been called, update the `reverted` status of
        // the previous call depth's recorded accesses, if any
        if let Some(recorded_account_diffs_stack) = &mut self.recorded_account_diffs_stack {
            // The root call cannot be recorded.
            if ecx.journaled_state.depth() > 0 {
                let mut last_recorded_depth = recorded_account_diffs_stack
                    .pop()
                    .expect("missing CALL account accesses");
                // Update the reverted status of all deeper calls if this call reverted, in
                // accordance with EVM behavior
                if outcome.result.is_revert() {
                    for element in last_recorded_depth.iter_mut() {
                        element.reverted = true;
                        element
                            .storageAccesses
                            .iter_mut()
                            .for_each(|storage_access| storage_access.reverted = true);
                    }
                }
                let call_access = last_recorded_depth
                    .first_mut()
                    .expect("empty AccountAccesses");
                // Assert that we're at the correct depth before recording post-call state
                // changes. Depending on the depth the cheat was called at,
                // there may not be any pending calls to update if execution has
                // percolated up to a higher depth.
                let curr_depth = ecx.journaled_state.depth() as u64;
                if call_access.depth == curr_depth {
                    if let Ok(acc) = ecx.journaled_state.load_account(call.target_address) {
                        debug_assert!(access_is_call(call_access.kind));
                        call_access.newBalance = acc.info.balance;
                    }
                }
                // Merge the last depth's AccountAccesses into the AccountAccesses at the
                // current depth, or push them back onto the pending vector if
                // higher depths were not recorded. This preserves ordering of
                // accesses.
                if let Some(last) = recorded_account_diffs_stack.last_mut() {
                    last.append(&mut last_recorded_depth);
                } else {
                    recorded_account_diffs_stack.push(last_recorded_depth);
                }
            }
        }

        // At the end of the call,
        // we need to check if we've found all the emits.
        // We know we've found all the expected emits in the right order
        // if the queue is fully matched.
        // If it's not fully matched, then either:
        // 1. Not enough events were emitted (we'll know this because the amount of
        //    times we
        // inspected events will be less than the size of the queue) 2. The wrong events
        // were emitted (The inspected events should match the size of the queue, but
        // still some events will not be matched)

        // First, check that we're at the call depth where the emits were declared from.
        let should_check_emits = self
            .expected_emits
            .iter()
            .any(|expected| {
                let curr_depth =
                    ecx.journaled_state.depth() as u64;
                expected.depth == curr_depth
            }) &&
            // Ignore staticcalls
            !call.is_static;
        if should_check_emits {
            // Not all emits were matched.
            if self.expected_emits.iter().any(|expected| !expected.found) {
                outcome.result.result = InstructionResult::Revert;
                outcome.result.output = "log != expected log".abi_encode().into();
                return;
            } else {
                // All emits were found, we're good.
                // Clear the queue, as we expect the user to declare more events for the next
                // call if they wanna match further events.
                self.expected_emits.clear();
            }
        }

        // this will ensure we don't have false positives when trying to diagnose
        // reverts in fork mode
        let diag = self.fork_revert_diagnostic.take();

        // if there's a revert and a previous call was diagnosed as fork related revert
        // then we can return a better error here
        if outcome.result.is_revert() {
            if let Some(err) = diag {
                outcome.result.output = Error::encode(err.to_error_msg(&self.labels));
                return;
            }
        }

        // try to diagnose reverts in multi-fork mode where a call is made to an address
        // that does not exist
        if let TxKind::Call(test_contract) = ecx.tx.kind() {
            // if a call to a different contract than the original test contract returned
            // with `Stop` we check if the contract actually exists on the
            // active fork
            if ecx.journaled_state.database.is_forked_mode()
                && outcome.result.result == InstructionResult::Stop
                && call.target_address != test_contract
            {
                self.fork_revert_diagnostic = ecx
                    .journaled_state
                    .database
                    .diagnose_revert(call.target_address, &ecx.journaled_state);
            }
        }

        // If the depth is 0, then this is the root call terminating
        if ecx.journaled_state.depth() == 0 {
            // If we already have a revert, we shouldn't run the below logic as it can
            // obfuscate an earlier error that happened first with unrelated
            // information about another error when using cheatcodes.
            if outcome.result.is_revert() {
                return;
            }

            // If there's not a revert, we can continue on to run the last logic for expect*
            // cheatcodes. Match expected calls
            for (address, calldatas) in &self.expected_calls {
                // Loop over each address, and for each address, loop over each calldata it
                // expects.
                for (calldata, (expected, actual_count)) in calldatas {
                    // Grab the values we expect to see
                    let ExpectedCallData {
                        gas,
                        min_gas,
                        value,
                        count,
                        call_type,
                    } = expected;

                    let failed = match call_type {
                        // If the cheatcode was called with a `count` argument,
                        // we must check that the EVM performed a CALL with this calldata exactly
                        // `count` times.
                        ExpectedCallType::Count => *count != *actual_count,
                        // If the cheatcode was called without a `count` argument,
                        // we must check that the EVM performed a CALL with this calldata at least
                        // `count` times. The amount of times to check was
                        // the amount of time the cheatcode was called.
                        ExpectedCallType::NonCount => *count > *actual_count,
                    };
                    if failed {
                        let expected_values = [
                            Some(format!("data {}", hex::encode_prefixed(calldata))),
                            value.as_ref().map(|v| format!("value {v}")),
                            gas.map(|g| format!("gas {g}")),
                            min_gas.map(|g| format!("minimum gas {g}")),
                        ]
                        .into_iter()
                        .flatten()
                        .join(", ");
                        let but = if outcome.result.is_ok() {
                            let s = if *actual_count == 1 { "" } else { "s" };
                            format!("was called {actual_count} time{s}")
                        } else {
                            "the call reverted instead; \
                             ensure you're testing the happy path when using `expectCall`"
                                .to_string()
                        };
                        let s = if *count == 1 { "" } else { "s" };
                        let msg = format!(
                            "expected call to {address} with {expected_values} \
                             to be called {count} time{s}, but {but}"
                        );
                        outcome.result.result = InstructionResult::Revert;
                        outcome.result.output = Error::encode(msg);

                        return;
                    }
                }
            }

            // Check if we have any leftover expected emits
            // First, if any emits were found at the root call, then we its ok and we remove
            // them.
            self.expected_emits.retain(|expected| !expected.found);
            // If not empty, we got mismatched emits
            if !self.expected_emits.is_empty() {
                let msg = if outcome.result.is_ok() {
                    "expected an emit, but no logs were emitted afterwards. \
                     you might have mismatched events or not enough events were emitted"
                } else {
                    "expected an emit, but the call reverted instead. \
                     ensure you're testing the happy path when using `expectEmit`"
                };
                outcome.result.result = InstructionResult::Revert;
                outcome.result.output = Error::encode(msg);
            }
        }
    }

    fn create(
        &mut self,
        ecx: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        call: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        let curr_depth = ecx.journaled_state.depth() as u64;

        // Apply our prank
        if let Some(prank) = &self.prank {
            if curr_depth >= prank.depth && call.caller == prank.prank_caller {
                // At the target depth we set `msg.sender`
                if curr_depth == prank.depth {
                    call.caller = prank.new_caller;
                }

                // At the target depth, or deeper, we set `tx.origin`
                if let Some(new_origin) = prank.new_origin {
                    ecx.tx.set_caller(new_origin);
                }
            }
        }

        // allow cheatcodes from the address of the new contract
        // Compute the address *after* any possible broadcast updates, so it's based on
        // the updated call inputs
        let address = self.allow_cheatcodes_on_create(ecx, call);
        // If `recordAccountAccesses` has been called, record the create
        if let Some(recorded_account_diffs_stack) = &mut self.recorded_account_diffs_stack {
            // Record the create context as an account access and create a new vector to
            // record all subsequent account accesses
            recorded_account_diffs_stack.push(vec![AccountAccess {
                chainInfo: crate::Vm::ChainInfo {
                    forkId: ecx
                        .journaled_state
                        .database
                        .active_fork_id()
                        .unwrap_or_default(),
                    chainId: U256::from(ecx.cfg.chain_id),
                },
                accessor: call.caller,
                account: address,
                kind: crate::Vm::AccountAccessKind::Create,
                initialized: true,
                oldBalance: U256::ZERO, // updated on create_end
                newBalance: U256::ZERO, // updated on create_end
                value: call.value,
                data: call.init_code.clone(),
                reverted: false,
                deployedCode: Bytes::new(), // updated on create_end
                storageAccesses: vec![],    // updated on create_end
                depth: curr_depth,
            }]);
        }

        None
    }

    fn create_end(
        &mut self,
        ecx: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        _call: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        let curr_depth = ecx.journaled_state.depth() as u64;

        // Clean up pranks
        if let Some(prank) = &self.prank {
            if curr_depth == prank.depth {
                ecx.tx.set_caller(prank.prank_origin);

                // Clean single-call prank once we have returned to the original depth
                if prank.single_call {
                    std::mem::take(&mut self.prank);
                }
            }
        }

        // Handle expected reverts
        if let Some(expected_revert) = &self.expected_revert {
            if curr_depth <= expected_revert.depth
                && matches!(expected_revert.kind, ExpectedRevertKind::Default)
            {
                let expected_revert = std::mem::take(&mut self.expected_revert).unwrap();
                return match expect::handle_expect_revert(
                    true,
                    expected_revert.reason.as_deref(),
                    outcome.result.result,
                    outcome.result.output.clone(),
                ) {
                    Ok((address, retdata)) => {
                        outcome.result.result = InstructionResult::Return;
                        outcome.result.output = retdata;
                        outcome.address = address;
                    }
                    Err(err) => {
                        outcome.result.result = InstructionResult::Revert;
                        outcome.result.output = err.abi_encode().into();
                    }
                };
            }
        }

        // If `startStateDiffRecording` has been called, update the `reverted` status of
        // the previous call depth's recorded accesses, if any
        if let Some(recorded_account_diffs_stack) = &mut self.recorded_account_diffs_stack {
            // The root call cannot be recorded.
            if ecx.journaled_state.depth() > 0 {
                let mut last_depth = recorded_account_diffs_stack
                    .pop()
                    .expect("missing CREATE account accesses");
                // Update the reverted status of all deeper calls if this call reverted, in
                // accordance with EVM behavior
                if outcome.result.is_revert() {
                    for element in last_depth.iter_mut() {
                        element.reverted = true;
                        element
                            .storageAccesses
                            .iter_mut()
                            .for_each(|storage_access| storage_access.reverted = true);
                    }
                }
                let create_access = last_depth.first_mut().expect("empty AccountAccesses");
                // Assert that we're at the correct depth before recording post-create state
                // changes. Depending on what depth the cheat was called at, there
                // may not be any pending calls to update if execution has
                // percolated up to a higher depth.
                if create_access.depth == ecx.journaled_state.depth() as u64 {
                    debug_assert_eq!(
                        create_access.kind as u8,
                        crate::Vm::AccountAccessKind::Create as u8
                    );
                    if let Some(address) = outcome.address {
                        if let Ok(created_acc) = ecx.journaled_state.load_account(address) {
                            create_access.newBalance = created_acc.info.balance;
                            create_access.deployedCode = created_acc
                                .info
                                .code
                                .clone()
                                .unwrap_or_default()
                                .original_bytes();
                        }
                    }
                }
                // Merge the last depth's AccountAccesses into the AccountAccesses at the
                // current depth, or push them back onto the pending vector if
                // higher depths were not recorded. This preserves ordering of
                // accesses.
                if let Some(last) = recorded_account_diffs_stack.last_mut() {
                    last.append(&mut last_depth);
                } else {
                    recorded_account_diffs_stack.push(last_depth);
                }
            }
        }
    }
}

/// Helper that expands memory, stores a revert string pertaining to a
/// disallowed memory write, and sets the return range to the revert string's
/// location in memory.
///
/// This will set the interpreter's next action to a return with the revert
/// string as the output. And trigger a revert.
fn disallowed_mem_write(
    dest_offset: u64,
    size: u64,
    interpreter: &mut Interpreter,
    ranges: &[Range<u64>],
) {
    let revert_string = format!(
        "memory write at offset 0x{:02X} of size 0x{:02X} not allowed; safe range: {}",
        dest_offset,
        size,
        ranges
            .iter()
            .map(|r| format!("(0x{:02X}, 0x{:02X}]", r.start, r.end))
            .join(" U ")
    );

    interpreter.control.instruction_result = InstructionResult::Revert;
    interpreter.control.next_action = InterpreterAction::Return {
        result: InterpreterResult {
            output: Error::encode(revert_string),
            gas: interpreter.control.gas,
            result: InstructionResult::Revert,
        },
    };
}

/// Dispatches the cheatcode call to the appropriate function.
fn apply_dispatch<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    HardforkT: HardforkTr,
    ChainContextT: ChainContextTr,
    DatabaseT: CheatcodeBackend<BlockT, TxT, HardforkT, ChainContextT>,
>(
    calls: &Vm::VmCalls,
    ccx: &mut CheatsCtxt<BlockT, TxT, HardforkT, ChainContextT, DatabaseT>,
) -> Result {
    macro_rules! match_ {
        ($($variant:ident),*) => {
            match calls {
                $(Vm::VmCalls::$variant(cheat) => crate::Cheatcode::apply_traced(cheat, ccx),)*
            }
        };
    }
    vm_calls!(match_)
}

/// Returns true if the kind of account access is a call.
fn access_is_call(kind: crate::Vm::AccountAccessKind) -> bool {
    matches!(
        kind,
        crate::Vm::AccountAccessKind::Call
            | crate::Vm::AccountAccessKind::StaticCall
            | crate::Vm::AccountAccessKind::CallCode
            | crate::Vm::AccountAccessKind::DelegateCall
    )
}

/// Appends an `AccountAccess` that resumes the recording of the current
/// context.
fn append_storage_access(
    accesses: &mut [Vec<AccountAccess>],
    storage_access: crate::Vm::StorageAccess,
    storage_depth: u64,
) {
    if let Some(last) = accesses.last_mut() {
        // Assert that there's an existing record for the current context.
        if !last.is_empty() && last.first().unwrap().depth < storage_depth {
            // Three cases to consider:
            // 1. If there hasn't been a context switch since the start of this context,
            //    then add the storage access to the current context record.
            // 2. If there's an existing Resume record, then add the storage access to it.
            // 3. Otherwise, create a new Resume record based on the current context.
            if last.len() == 1 {
                last.first_mut()
                    .unwrap()
                    .storageAccesses
                    .push(storage_access);
            } else {
                let last_record = last.last_mut().unwrap();
                if last_record.kind as u8 == crate::Vm::AccountAccessKind::Resume as u8 {
                    last_record.storageAccesses.push(storage_access);
                } else {
                    let entry = last.first().unwrap();
                    let resume_record = crate::Vm::AccountAccess {
                        chainInfo: crate::Vm::ChainInfo {
                            forkId: entry.chainInfo.forkId,
                            chainId: entry.chainInfo.chainId,
                        },
                        accessor: entry.accessor,
                        account: entry.account,
                        kind: crate::Vm::AccountAccessKind::Resume,
                        initialized: entry.initialized,
                        storageAccesses: vec![storage_access],
                        reverted: entry.reverted,
                        // The remaining fields are defaults
                        oldBalance: U256::ZERO,
                        newBalance: U256::ZERO,
                        value: U256::ZERO,
                        data: Bytes::new(),
                        deployedCode: Bytes::new(),
                        depth: entry.depth,
                    };
                    last.push(resume_record);
                }
            }
        }
    }
}

fn find_upstream_cheatcode_signature(selector: alloy_primitives::FixedBytes<4>) -> Option<String> {
    for (_function_name, variants) in UpstreamVM::abi::functions() {
        for abi_function in variants {
            if abi_function.selector() == selector {
                return Some(abi_function.signature());
            }
        }
    }
    None
}
