use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use alloy_primitives::{
    map::{AddressHashMap, HashMap},
    Address, Bytes, Log, TxKind, U256,
};
use derive_where::derive_where;
use edr_coverage::CodeCoverageReporter;
use eyre::eyre;
use foundry_cheatcodes::CheatcodesExecutor;
use foundry_evm_core::{
    backend::{CheatcodeBackend, JournaledState},
    evm_context::{
        split_context_deref_mut, BlockEnvTr, ChainContextTr, EvmBuilderTrait, EvmEnv, HardforkTr,
        IntoEvmContext as _, TransactionEnvTr, TransactionErrorTrait,
    },
};
use foundry_evm_coverage::HitMaps;
use foundry_evm_traces::{SparsedTraceArena, TracingMode};
use revm::{
    context::{
        result::{ExecutionResult, HaltReason, HaltReasonTr},
        BlockEnv, CfgEnv, Context as EvmContext, CreateScheme,
    },
    context_interface::{result::Output, JournalTr},
    interpreter::{
        interpreter::EthInterpreter, CallInputs, CallOutcome, CallScheme, CreateInputs,
        CreateOutcome, Gas, InstructionResult, Interpreter, InterpreterResult,
    },
    state::{Account, AccountStatus},
    DatabaseCommit, InspectEvm as _, Inspector, Journal, JournalEntry,
};
use revm_inspectors::edge_cov::EdgeCovInspector;

use super::{
    Cheatcodes, CheatsConfig, Fuzzer, LineCoverageCollector, LogCollector, RevertDiagnostic,
    TracingInspector,
};

#[derive(Clone, Debug, Default)]
#[must_use = "builders do nothing unless you call `build` on them"]
pub struct InspectorStackBuilder<HardforkT: HardforkTr, ChainContextT: ChainContextTr> {
    /// The block environment.
    ///
    /// Used in the cheatcode handler to overwrite the block environment
    /// separately from the execution block environment.
    pub block: Option<BlockEnv>,
    /// The multichain context
    pub chain_context: Option<ChainContextT>,
    /// EDR coverage reporter.
    pub code_coverage: Option<CodeCoverageReporter>,
    /// The gas price.
    ///
    /// Used in the cheatcode handler to overwrite the gas price separately from
    /// the gas price in the execution environment.
    pub gas_price: Option<u128>,
    /// The cheatcodes config.
    pub cheatcodes: Option<Arc<CheatsConfig<HardforkT>>>,
    /// The fuzzer inspector and its state, if it exists.
    pub fuzzer: Option<Fuzzer>,
    /// Whether to enable tracing.
    pub trace: Option<TracingMode>,
    /// Whether logs should be collected.
    pub logs: Option<bool>,
    /// Whether line coverage info should be collected.
    pub line_coverage: Option<bool>,
    /// Whether to enable call isolation.
    /// In isolation mode all top-level calls are executed as a separate
    /// transaction in a separate EVM context, enabling more precise gas
    /// accounting and transaction state changes.
    pub enable_isolation: bool,
}

impl<HardforkT: HardforkTr, ChainContextT: ChainContextTr>
    InspectorStackBuilder<HardforkT, ChainContextT>
{
    /// Create a new inspector stack builder.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the block environment.
    #[inline]
    pub fn block(mut self, block: BlockEnv) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the gas price.
    #[inline]
    pub fn gas_price(mut self, gas_price: u128) -> Self {
        self.gas_price = Some(gas_price);
        self
    }

    /// Enable cheatcodes with the given config.
    #[inline]
    pub fn cheatcodes(mut self, config: Arc<CheatsConfig<HardforkT>>) -> Self {
        self.cheatcodes = Some(config);
        self
    }

    /// Set the fuzzer inspector.
    #[inline]
    pub fn fuzzer(mut self, fuzzer: Fuzzer) -> Self {
        self.fuzzer = Some(fuzzer);
        self
    }

    /// Set whether to collect logs.
    #[inline]
    pub fn logs(mut self, yes: bool) -> Self {
        self.logs = Some(yes);
        self
    }

    /// Set whether to report EDR code coverage.
    #[inline]
    pub fn code_coverage(mut self, reporter: Option<CodeCoverageReporter>) -> Self {
        if let Some(reporter) = reporter {
            self.code_coverage = Some(reporter);
        }
        self
    }

    /// Set whether to collect coverage information.
    #[inline]
    pub fn coverage(mut self, yes: bool) -> Self {
        self.line_coverage = Some(yes);
        self
    }

    /// Set whether to enable the tracer.
    #[inline]
    pub fn trace(mut self, mode: TracingMode) -> Self {
        self.trace = Some(mode);
        self
    }

    /// Set whether to enable the call isolation.
    /// For description of call isolation, see
    /// [`InspectorStack::enable_isolation`].
    #[inline]
    pub fn enable_isolation(mut self, yes: bool) -> Self {
        self.enable_isolation = yes;
        self
    }

    /// Builds the stack of inspectors to use when transacting/committing on the
    /// EVM.
    ///
    /// See also [`revm::Evm::inspect_ref`] and [`revm::Evm::commit_ref`].
    pub fn build<
        BlockT: BlockEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        TransactionErrorT: TransactionErrorTrait,
        TxT: TransactionEnvTr,
    >(
        self,
    ) -> InspectorStack<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    > {
        let Self {
            block,
            chain_context,
            code_coverage,
            gas_price,
            cheatcodes,
            fuzzer,
            trace,
            logs,
            line_coverage,
            enable_isolation,
        } = self;
        let mut stack = InspectorStack::new();

        // inspectors
        if let Some(config) = cheatcodes {
            stack.set_cheatcodes(Cheatcodes::new(config));
        }

        if let Some(fuzzer) = fuzzer {
            stack.set_fuzzer(fuzzer);
        }
        if let Some(reporter) = code_coverage {
            stack.set_code_coverage(reporter);
        }
        stack.collect_line_coverage(line_coverage.unwrap_or(false));
        stack.collect_logs(logs.unwrap_or(true));
        stack.tracing(trace.unwrap_or(TracingMode::None));

        stack.enable_isolation(enable_isolation);

        // environment, must come after all of the inspectors
        if let Some(block) = block {
            stack.set_block(block);
        }
        if let Some(chain_context) = chain_context {
            stack.set_chain_context(chain_context);
        }
        if let Some(gas_price) = gas_price {
            stack.set_gas_price(gas_price);
        }

        stack
    }
}

/// Helper macro to call the same method on multiple inspectors without
/// resorting to dynamic dispatch.
#[macro_export]
macro_rules! call_inspectors {
    ([$($inspector:expr),+ $(,)?], |$id:ident $(,)?| $body:expr $(,)?) => {
        $(
            if let Some($id) = $inspector {
                $crate::utils::cold_path();
                $body;
            }
        )+
    };
    (#[ret] [$($inspector:expr),+ $(,)?], |$id:ident $(,)?| $body:expr $(,)?) => {{
        $(
            if let Some($id) = $inspector {
                $crate::utils::cold_path();
                if let Some(result) = $body {
                    return result;
                }
            }
        )+
    }};
}

/// The collected results of [`InspectorStack`].
pub struct InspectorData<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
> {
    pub logs: Vec<Log>,
    pub labels: AddressHashMap<String>,
    pub traces: Option<SparsedTraceArena>,
    pub line_coverage: Option<HitMaps>,
    pub edge_coverage: Option<Vec<u8>>,
    pub cheatcodes: Option<
        Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    >,
    pub reverter: Option<Address>,
}

/// Contains data about the state of outer/main EVM which created and invoked
/// the inner EVM context. Used to adjust EVM state while in inner context.
///
/// We need this to avoid breaking changes due to EVM behavior differences in
/// isolated vs non-isolated mode. For descriptions and workarounds for those changes see: <https://github.com/foundry-rs/foundry/pull/7186#issuecomment-1959102195>
#[derive(Debug, Clone)]
pub struct InnerContextData {
    /// Origin of the transaction in the outer EVM context.
    original_origin: Address,
}

/// An inspector that calls multiple inspectors in sequence.
///
/// If a call to an inspector returns a value (indicating a stop or revert) the
/// remaining inspectors are not called.
///
/// Stack is divided into [Cheatcodes] and `InspectorStackInner`. This is done
/// to allow assembling `InspectorStackRefMut` inside [Cheatcodes] to allow
/// usage of it as [`revm::Inspector`]. This gives us ability to create and
/// execute separate EVM frames from inside cheatcodes while still having access
/// to entire stack of inspectors and correctly handling traces, logs, debugging
/// info collection, etc.
#[derive_where(Clone, Debug, Default; BlockT, TxT, HardforkT)]
pub struct InspectorStack<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
> {
    pub cheatcodes: Option<
        Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    >,
    pub inner: InspectorStackInner<ChainContextT>,
}

/// All used inpectors besides [Cheatcodes].
///
/// See [`InspectorStack`].
#[derive(Default, Clone, Debug)]
pub struct InspectorStackInner<ChainContextT> {
    // Inspectors.
    pub edge_coverage: Option<EdgeCovInspector>,
    pub fuzzer: Option<Fuzzer>,
    /// EDR coverage reporter.
    pub code_coverage: Option<CodeCoverageReporter>,
    pub line_coverage: Option<LineCoverageCollector>,
    pub log_collector: Option<LogCollector>,
    pub revert_diag: Option<RevertDiagnostic>,
    pub tracer: Option<TracingInspector>,

    // InspectorExt and other internal data.
    pub enable_isolation: bool,
    /// Flag marking if we are in the inner EVM context.
    pub in_inner_context: bool,
    pub inner_context_data: Option<InnerContextData>,
    pub top_frame_journal: HashMap<Address, Account>,
    /// Address that reverted the call, if any.
    pub reverter: Option<Address>,
    pub chain_context: ChainContextT,
}

/// Struct keeping mutable references to both parts of [`InspectorStack`] and
/// implementing [`revm::Inspector`]. This struct can be obtained via
/// [`InspectorStack::as_mut`] or via [`CheatcodesExecutor::get_inspector`]
/// method implemented for [`InspectorStackInner`].
pub struct InspectorStackRefMut<
    'a,
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
> {
    pub cheatcodes: Option<
        &'a mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    >,
    pub inner: &'a mut InspectorStackInner<ChainContextT>,
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
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
    >
    CheatcodesExecutor<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    > for InspectorStackInner<ChainContextT>
{
    fn tracing_inspector(&mut self) -> Option<&mut Option<TracingInspector>> {
        Some(&mut self.tracer)
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
    InspectorStack<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    /// Creates a new inspector stack.
    ///
    /// Note that the stack is empty by default, and you must add inspectors to
    /// it. This is done by calling the `set_*` methods on the stack
    /// directly, or by building the stack with [`InspectorStack`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set variables from an environment for the relevant inspectors.
    #[inline]
    pub fn set_env(&mut self, env: &EvmEnv<BlockT, TxT, HardforkT>) {
        self.set_block(env.block.clone().into());
        self.set_gas_price(env.tx.gas_price());
    }

    /// Sets the block for the relevant inspectors.
    #[inline]
    pub fn set_block(&mut self, block: BlockEnv) {
        if let Some(cheatcodes) = &mut self.cheatcodes {
            cheatcodes.block = Some(block);
        }
    }

    /// Sets the multichain context.
    #[inline]
    pub fn set_chain_context(&mut self, chain_context: ChainContextT) {
        self.chain_context = chain_context;
    }

    /// Sets the gas price for the relevant inspectors.
    #[inline]
    pub fn set_gas_price(&mut self, gas_price: u128) {
        if let Some(cheatcodes) = &mut self.cheatcodes {
            cheatcodes.gas_price = Some(gas_price);
        }
    }

    /// Set the cheatcodes inspector.
    #[inline]
    pub fn set_cheatcodes(
        &mut self,
        cheatcodes: Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) {
        self.cheatcodes = Some(cheatcodes);
    }

    /// Set the fuzzer inspector.
    #[inline]
    pub fn set_fuzzer(&mut self, fuzzer: Fuzzer) {
        self.fuzzer = Some(fuzzer);
    }

    /// Set whether to enable EDR code coverage reporting.
    #[inline]
    pub fn set_code_coverage(&mut self, reporter: CodeCoverageReporter) {
        self.code_coverage = Some(reporter);
    }

    /// Set whether to enable the line coverage collector.
    #[inline]
    pub fn collect_line_coverage(&mut self, yes: bool) {
        self.line_coverage = yes.then(Default::default);
    }

    /// Set whether to enable the edge coverage collector.
    #[inline]
    pub fn collect_edge_coverage(&mut self, yes: bool) {
        self.edge_coverage = yes.then(EdgeCovInspector::new); // TODO configurable edge size?
    }

    /// Set whether to enable call isolation.
    #[inline]
    pub fn enable_isolation(&mut self, yes: bool) {
        self.enable_isolation = yes;
    }

    /// Set whether to enable the log collector.
    #[inline]
    pub fn collect_logs(&mut self, yes: bool) {
        self.log_collector = yes.then(Default::default);
    }

    /// Set whether to enable the tracer.
    /// Revert diagnostic inspector is activated when `mode != TraceMode::None`
    #[inline]
    pub fn tracing(&mut self, mode: TracingMode) {
        if matches!(mode, TracingMode::None) {
            self.revert_diag = None;
        } else {
            self.revert_diag = Some(RevertDiagnostic::default());
        }

        if let Some(config) = mode.into_config() {
            *self
                .tracer
                .get_or_insert_with(Default::default)
                .config_mut() = config;
        } else {
            self.tracer = None;
        }
    }

    /// Collects all the data gathered during inspection into a single struct.
    #[inline]
    pub fn collect(
        self,
    ) -> eyre::Result<
        InspectorData<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    > {
        let Self {
            mut cheatcodes,
            inner:
                InspectorStackInner {
                    code_coverage,
                    line_coverage,
                    edge_coverage,
                    log_collector,
                    tracer,
                    reverter,
                    ..
                },
        } = self;

        let traces = tracer
            .map(foundry_evm_traces::TracingInspector::into_traces)
            .map(|arena| {
                let ignored = cheatcodes
                    .as_mut()
                    .map(|cheatcodes| {
                        let mut ignored = std::mem::take(&mut cheatcodes.ignored_traces.ignored);

                        // If the last pause call was not resumed, ignore the rest of the trace
                        if let Some(last_pause_call) = cheatcodes.ignored_traces.last_pause_call {
                            ignored.insert(last_pause_call, (arena.nodes().len(), 0));
                        }

                        ignored
                    })
                    .unwrap_or_default();

                SparsedTraceArena { arena, ignored }
            });

        if let Some(code_coverage) = code_coverage {
            code_coverage.report().map_err(|error| eyre!(error))?;
        }

        Ok(InspectorData {
            logs: log_collector.map(|logs| logs.logs).unwrap_or_default(),
            labels: cheatcodes
                .as_ref()
                .map(|cheatcodes| cheatcodes.labels.clone())
                .unwrap_or_default(),
            traces,
            line_coverage: line_coverage.map(foundry_evm_coverage::LineCoverageCollector::finish),
            edge_coverage: edge_coverage
                .map(revm_inspectors::edge_cov::EdgeCovInspector::into_hitcount),
            cheatcodes,
            reverter,
        })
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr + TryInto<HaltReason>,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
    >
    InspectorStack<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    #[inline(always)]
    fn as_mut(
        &mut self,
    ) -> InspectorStackRefMut<
        '_,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    > {
        InspectorStackRefMut {
            cheatcodes: self.cheatcodes.as_mut(),
            inner: &mut self.inner,
        }
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr + TryInto<HaltReason>,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
    >
    InspectorStackRefMut<
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
    /// Adjusts the EVM data for the inner EVM context.
    /// Should be called on the top-level call of inner context (depth == 0 &&
    /// `self.in_inner_context`) Decreases sender nonce for CALLs to keep
    /// backwards compatibility Updates tx.origin to the value before
    /// entering inner context
    fn adjust_evm_data_for_inner_context<
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
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
        let inner_context_data = self
            .inner_context_data
            .as_ref()
            .expect("should be called in inner context");
        ecx.tx.set_caller(inner_context_data.original_origin);
    }

    fn do_call_end<
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
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
        inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) -> CallOutcome {
        let result = outcome.result.result;
        call_inspectors!(
            #[ret]
            [
                &mut self.fuzzer,
                &mut self.tracer,
                &mut self.cheatcodes,
                &mut self.revert_diag
            ],
            |inspector| {
                let previous_outcome = outcome.clone();
                inspector.call_end(ecx, inputs, outcome);

                // If the inspector returns a different status or a revert with a non-empty
                // message, we assume it wants to tell us something
                let different = outcome.result.result != result
                    || (outcome.result.result == InstructionResult::Revert
                        && outcome.output() != previous_outcome.output());
                different.then_some(outcome.clone())
            },
        );

        // Record first address that reverted the call.
        if result.is_revert() && self.reverter.is_none() {
            self.reverter = Some(inputs.target_address);
        }

        outcome.clone()
    }

    fn do_create_end<
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
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
        call: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) -> CreateOutcome {
        let result = outcome.result.result;
        call_inspectors!(
            #[ret]
            [&mut self.tracer, &mut self.cheatcodes],
            |inspector| {
                let previous_outcome = outcome.clone();
                inspector.create_end(ecx, call, outcome);

                // If the inspector returns a different status or a revert with a non-empty
                // message, we assume it wants to tell us something
                let different = outcome.result.result != result
                    || (outcome.result.result == InstructionResult::Revert
                        && outcome.output() != previous_outcome.output());
                different.then_some(outcome.clone())
            },
        );

        outcome.clone()
    }

    fn transact_inner<
        DatabaseT: CheatcodeBackend<
                BlockT,
                TxT,
                EvmBuilderT,
                HaltReasonT,
                HardforkT,
                TransactionErrorT,
                ChainContextT,
            > + DatabaseCommit
            + DerefMut<
                Target: CheatcodeBackend<
                    BlockT,
                    TxT,
                    EvmBuilderT,
                    HaltReasonT,
                    HardforkT,
                    TransactionErrorT,
                    ChainContextT,
                > + DatabaseCommit
                            + Sized,
            >,
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
        kind: TxKind,
        caller: Address,
        input: Bytes,
        gas_limit: u64,
        value: U256,
    ) -> (InterpreterResult, Option<Address>) {
        let cached_env = EvmEnv::new(ecx.cfg.clone(), ecx.block.clone(), ecx.tx.clone());

        ecx.block.set_basefee(0);
        ecx.tx.set_chain_id(Some(ecx.cfg.chain_id));
        ecx.tx.set_caller(caller);
        ecx.tx.set_kind(kind);
        ecx.tx.set_data(input);
        ecx.tx.set_value(value);
        // Add 21000 to the gas limit to account for the base cost of transaction.
        ecx.tx.set_gas_limit(gas_limit + 21000);

        // If we haven't disabled gas limit checks, ensure that transaction gas limit
        // will not exceed block gas limit.
        if !ecx.cfg.disable_block_gas_limit {
            ecx.tx
                .set_gas_limit(std::cmp::min(ecx.tx.gas_limit(), ecx.block.gas_limit()));
        }
        ecx.tx.set_gas_price(0);

        self.inner_context_data = Some(InnerContextData {
            original_origin: cached_env.tx.caller(),
        });
        self.in_inner_context = true;

        let res = self.with_stack(|inspector| {
            let (db, context) = split_context_deref_mut(ecx);

            let state = {
                let mut state = context.journaled_state.state.clone();

                for (addr, acc_mut) in &mut state {
                    // mark all accounts cold, besides preloaded addresses
                    if context.journaled_state.warm_addresses.is_cold(addr) {
                        acc_mut.mark_cold();
                    }

                    // mark all slots cold
                    for slot_mut in acc_mut.storage.values_mut() {
                        slot_mut.is_cold = true;
                        slot_mut.original_value = slot_mut.present_value;
                    }
                }

                state
            };

            let mut journaled_state = Journal::<_, JournalEntry>::new(db);
            journaled_state.state = state;
            journaled_state.set_spec_id(context.cfg.spec.into());
            // set depth to 1 to make sure traces are collected correctly
            journaled_state.depth = 1;

            let env_with_chain = context.to_owned_env_with_chain_context();
            let mut evm = EvmBuilderT::evm_with_journal_and_inspector(
                journaled_state,
                env_with_chain,
                inspector,
            );

            let res = evm.inspect_tx(context.tx.clone());

            // need to reset the env in case it was modified via cheatcodes during execution
            let evm_context = evm.into_evm_context();
            *context.cfg = evm_context.cfg.clone();
            *context.block = evm_context.block.clone();

            *context.tx = cached_env.tx;
            context.block.set_basefee(cached_env.block.basefee());

            res
        });

        self.in_inner_context = false;
        self.inner_context_data = None;

        let mut gas = Gas::new(gas_limit);

        let Ok(res) = res else {
            // Should we match, encode and propagate error as a revert reason?
            let result = InterpreterResult {
                result: InstructionResult::Revert,
                output: Bytes::new(),
                gas,
            };
            return (result, None);
        };

        for (addr, mut acc) in res.state {
            let Some(acc_mut) = ecx.journaled_state.state.get_mut(&addr) else {
                ecx.journaled_state.state.insert(addr, acc);
                continue;
            };

            // make sure accounts that were warmed earlier do not become cold
            if acc.status.contains(AccountStatus::Cold)
                && !acc_mut.status.contains(AccountStatus::Cold)
            {
                acc.status -= AccountStatus::Cold;
            }
            acc_mut.info = acc.info;
            acc_mut.status |= acc.status;

            for (key, val) in acc.storage {
                let Some(slot_mut) = acc_mut.storage.get_mut(&key) else {
                    acc_mut.storage.insert(key, val);
                    continue;
                };
                slot_mut.present_value = val.present_value;
                slot_mut.is_cold &= val.is_cold;
            }
        }

        let (result, address, output) = match res.result {
            ExecutionResult::Success {
                reason,
                gas_used,
                gas_refunded,
                logs: _,
                output,
            } => {
                gas.set_refund(gas_refunded as i64);
                let _ = gas.record_cost(gas_used);
                let address = match output {
                    Output::Create(_, address) => address,
                    Output::Call(_) => None,
                };
                (reason.into(), address, output.into_data())
            }
            ExecutionResult::Halt { reason, gas_used } => {
                let reason: HaltReason = reason.clone().try_into().unwrap_or_else(|_error| {
                    panic!("Halt reason cannot be converted to `HaltReason`: {reason:?}")
                });

                let _ = gas.record_cost(gas_used);
                (reason.into(), None, Bytes::new())
            }
            ExecutionResult::Revert { gas_used, output } => {
                let _ = gas.record_cost(gas_used);
                (InstructionResult::Revert, None, output)
            }
        };
        (
            InterpreterResult {
                result,
                output,
                gas,
            },
            address,
        )
    }

    /// Moves out of references, constructs an [`InspectorStack`] and runs the
    /// given closure with it.
    fn with_stack<O>(
        &mut self,
        f: impl FnOnce(
            &mut InspectorStack<
                BlockT,
                TxT,
                EvmBuilderT,
                HaltReasonT,
                HardforkT,
                TransactionErrorT,
                ChainContextT,
            >,
        ) -> O,
    ) -> O {
        let mut stack = InspectorStack {
            cheatcodes: self
                .cheatcodes
                .as_deref_mut()
                .map(|cheats| core::mem::replace(cheats, Cheatcodes::new(cheats.config.clone()))),
            inner: std::mem::take(self.inner),
        };

        let out = f(&mut stack);

        if let Some(cheats) = self.cheatcodes.as_deref_mut() {
            *cheats = stack.cheatcodes.take().unwrap();
        }

        *self.inner = stack.inner;

        out
    }

    /// Invoked at the beginning of a new top-level (0 depth) frame.
    fn top_level_frame_start<
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
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
        if self.enable_isolation {
            // If we're in isolation mode, we need to keep track of the state at the
            // beginning of the frame to be able to roll back on revert
            self.top_frame_journal
                .clone_from(&ecx.journaled_state.state);
        }
    }

    /// Invoked at the end of root frame.
    fn top_level_frame_end<
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
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
        result: InstructionResult,
    ) {
        if !result.is_revert() {
            return;
        }
        // Encountered a revert, since cheatcodes may have altered the evm state in such
        // a way that violates some constraints, e.g. `deal`, we need to
        // manually roll back on revert before revm reverts the state itself
        if let Some(cheats) = self.cheatcodes.as_mut() {
            cheats.on_revert(ecx);
        }

        // If we're in isolation mode, we need to rollback to state before the root
        // frame was created We can't rely on revm's journal because it doesn't
        // account for changes made by isolated calls
        if self.enable_isolation {
            ecx.journaled_state.state = std::mem::take(&mut self.top_frame_journal);
        }
    }

    #[inline(always)]
    fn step_inlined<
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
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
        call_inspectors!(
            [
                &mut self.fuzzer,
                &mut self.tracer,
                &mut self.line_coverage,
                &mut self.edge_coverage,
                &mut self.revert_diag,
                // Keep `cheatcodes` last to make use of the tail call.
                &mut self.cheatcodes,
            ],
            |inspector| (*inspector).step(interpreter, ecx),
        );
    }

    #[inline(always)]
    fn step_end_inlined<
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
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
        call_inspectors!(
            [
                &mut self.tracer,
                &mut self.revert_diag,
                // Keep `cheatcodes` last to make use of the tail call.
                &mut self.cheatcodes,
            ],
            |inspector| (*inspector).step_end(interpreter, ecx),
        );
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr + TryInto<HaltReason>,
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
            > + DatabaseCommit
            + DerefMut<
                Target: CheatcodeBackend<
                    BlockT,
                    TxT,
                    EvmBuilderT,
                    HaltReasonT,
                    HardforkT,
                    TransactionErrorT,
                    ChainContextT,
                > + DatabaseCommit
                            + Sized,
            >,
    >
    Inspector<
        EvmContext<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, Journal<DatabaseT>, ChainContextT>,
    >
    for InspectorStackRefMut<
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
    fn initialize_interp(
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
        call_inspectors!(
            [
                &mut self.line_coverage,
                &mut self.tracer,
                &mut self.cheatcodes,
            ],
            |inspector| inspector.initialize_interp(interpreter, ecx),
        );
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
        self.step_inlined(interpreter, ecx);
    }

    fn step_end(
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
        self.step_end_inlined(interpreter, ecx);
    }

    #[allow(clippy::redundant_clone)]
    fn log_full(
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
        log: Log,
    ) {
        call_inspectors!(
            [
                &mut self.tracer,
                &mut self.log_collector,
                &mut self.cheatcodes
            ],
            |inspector| inspector.log_full(interpreter, ecx, log.clone()),
        );
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
        if self.in_inner_context && ecx.journaled_state.depth == 1 {
            self.adjust_evm_data_for_inner_context(ecx);
            return None;
        }

        if ecx.journaled_state.depth == 0 {
            self.top_level_frame_start(ecx);
        }

        let code_coverage_collector = self
            .inner
            .code_coverage
            .as_mut()
            .map(|reporter| &mut reporter.collector);

        call_inspectors!(
            #[ret]
            [
                &mut self.inner.fuzzer,
                &mut self.inner.tracer,
                code_coverage_collector,
                &mut self.inner.log_collector,
                &mut self.inner.revert_diag
            ],
            |inspector| {
                let mut out = None;
                if let Some(output) = Inspector::<_, EthInterpreter>::call(inspector, ecx, call) {
                    out = Some(Some(output));
                }
                out
            },
        );

        if let Some(cheatcodes) = self.cheatcodes.as_deref_mut() {
            // Handle mocked functions, replace bytecode address with mock if matched.
            if let Some(mocks) = cheatcodes.mocked_functions.get(&call.target_address) {
                // Check if any mock function set for call data or if catch-all mock function
                // set for selector.
                if let Some(target) = mocks.get(&call.input.bytes(ecx)).or_else(|| {
                    call.input
                        .bytes(ecx)
                        .get(..4)
                        .and_then(|selector| mocks.get(selector))
                }) {
                    call.bytecode_address = *target;
                    call.known_bytecode = None;
                }
            }

            if let Some(output) = cheatcodes.call_with_executor(ecx, call, self.inner) {
                return Some(output);
            }
        }

        if self.enable_isolation && !self.in_inner_context && ecx.journaled_state.depth == 1 {
            match call.scheme {
                // Isolate CALLs
                CallScheme::Call => {
                    let input = call.input.bytes(ecx);
                    let (result, _) = self.transact_inner(
                        ecx,
                        TxKind::Call(call.target_address),
                        call.caller,
                        input,
                        call.gas_limit,
                        call.value.get(),
                    );
                    return Some(CallOutcome {
                        result,
                        memory_offset: call.return_memory_offset.clone(),
                        was_precompile_called: true,
                        precompile_call_logs: vec![],
                    });
                }
                // Mark accounts and storage cold before STATICCALLs
                CallScheme::StaticCall => {
                    let JournaledState {
                        state,
                        warm_addresses,
                        ..
                    } = &mut ecx.journaled_state.inner;
                    for (addr, acc_mut) in state {
                        // Do not mark accounts and storage cold accounts with arbitrary storage.
                        if let Some(cheatcodes) = &self.cheatcodes
                            && cheatcodes.has_arbitrary_storage(addr)
                        {
                            continue;
                        }

                        if warm_addresses.is_cold(addr) {
                            acc_mut.mark_cold();
                        }

                        for slot_mut in acc_mut.storage.values_mut() {
                            slot_mut.is_cold = true;
                        }
                    }
                }
                // Process other variants as usual
                CallScheme::CallCode | CallScheme::DelegateCall => {}
            }
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
        inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        // We are processing inner context outputs in the outer context, so need to
        // avoid processing twice.
        if self.in_inner_context && ecx.journaled_state.depth == 1 {
            return;
        }

        self.do_call_end(ecx, inputs, outcome);

        if ecx.journaled_state.depth == 0 {
            self.top_level_frame_end(ecx, outcome.result.result);
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
        create: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        if self.in_inner_context && ecx.journaled_state.depth == 1 {
            self.adjust_evm_data_for_inner_context(ecx);
            return None;
        }

        if ecx.journaled_state.depth == 0 {
            self.top_level_frame_start(ecx);
        }

        call_inspectors!(
            #[ret]
            [
                &mut self.tracer,
                &mut self.line_coverage,
                &mut self.cheatcodes
            ],
            |inspector| inspector.create(ecx, create).map(Some),
        );

        if !matches!(create.scheme, CreateScheme::Create2 { .. })
            && self.enable_isolation
            && !self.in_inner_context
            && ecx.journaled_state.depth == 1
        {
            let (result, address) = self.transact_inner(
                ecx,
                TxKind::Create,
                create.caller,
                create.init_code.clone(),
                create.gas_limit,
                create.value,
            );
            return Some(CreateOutcome { result, address });
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
        call: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        // We are processing inner context outputs in the outer context, so need to
        // avoid processing twice.
        if self.in_inner_context && ecx.journaled_state.depth == 1 {
            return;
        }

        self.do_create_end(ecx, call, outcome);

        if ecx.journaled_state.depth == 0 {
            self.top_level_frame_end(ecx, outcome.result.result);
        }
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr + TryInto<HaltReason>,
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
            > + DatabaseCommit
            + DerefMut<
                Target: CheatcodeBackend<
                    BlockT,
                    TxT,
                    EvmBuilderT,
                    HaltReasonT,
                    HardforkT,
                    TransactionErrorT,
                    ChainContextT,
                > + DatabaseCommit
                            + Sized,
            >,
    >
    Inspector<
        EvmContext<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, Journal<DatabaseT>, ChainContextT>,
    >
    for InspectorStack<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
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
        self.as_mut().step_inlined(interpreter, ecx);
    }

    fn step_end(
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
        self.as_mut().step_end_inlined(interpreter, ecx);
    }

    fn call(
        &mut self,
        context: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        self.as_mut().call(context, inputs)
    }

    fn call_end(
        &mut self,
        context: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        self.as_mut().call_end(context, inputs, outcome);
    }

    fn create(
        &mut self,
        context: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        create: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        self.as_mut().create(context, create)
    }

    fn create_end(
        &mut self,
        context: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        call: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.as_mut().create_end(context, call, outcome);
    }

    fn initialize_interp(
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
        self.as_mut().initialize_interp(interpreter, ecx);
    }

    fn log_full(
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
        log: Log,
    ) {
        self.as_mut().log_full(interpreter, ecx, log);
    }

    fn selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
        <InspectorStackRefMut<
            '_,
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        > as Inspector<
            revm::Context<
                BlockT,
                TxT,
                CfgEnv<HardforkT>,
                DatabaseT,
                Journal<DatabaseT>,
                ChainContextT,
            >,
        >>::selfdestruct(&mut self.as_mut(), contract, target, value);
    }
}

impl<
        'a,
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
    > Deref
    for InspectorStackRefMut<
        'a,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    type Target = &'a mut InspectorStackInner<ChainContextT>;

    fn deref(&self) -> &Self::Target {
        &self.inner
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
    > DerefMut
    for InspectorStackRefMut<
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
    #[allow(clippy::mut_mut)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
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
    > Deref
    for InspectorStack<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    type Target = InspectorStackInner<ChainContextT>;

    fn deref(&self) -> &Self::Target {
        &self.inner
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
    > DerefMut
    for InspectorStack<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
