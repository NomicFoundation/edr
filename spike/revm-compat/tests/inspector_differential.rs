//! Differential test for `InspectorBridge`: a revm@41 inspector driven by
//! op-revm (revm@38) through the bridge must observe exactly what a native
//! revm@38 inspector observes — and its mutations (gas) must take effect.

use op_revm::{DefaultOp, OpBuilder, OpSpecId, OpTransaction};
use revm38::{
    context::TxEnv as TxEnv38, database::CacheDB as CacheDB38,
    database_interface::EmptyDB as EmptyDB38, inspector::InspectEvm as _, Context as Context38,
};
use revm41::{
    database::CacheDB as CacheDB41,
    database_interface::EmptyDB as EmptyDB41,
    primitives::{Address, Bytes, TxKind, B256, KECCAK_EMPTY, U256},
};
use revm_compat_spike::{
    convert, db_bridge::DbBridge, hardfork::OpHardfork, inspector_bridge::InspectorBridge,
};

const CHAIN_ID: u64 = 10;
const ALICE: Address = Address::new([0xaa; 20]);
const BOB: Address = Address::new([0xbb; 20]);
const CONTRACT: Address = Address::new([0xc0; 20]);

/// Runtime code exercising storage, logs, an inner CALL, memory, and RETURN:
///   PUSH1 42 PUSH1 0 SSTORE                  slot0 = 42
///   PUSH1 0 PUSH1 0 LOG0                     empty log
///   PUSH1 0 (out_len) PUSH1 0 (out_off)
///   PUSH1 0 (in_len)  PUSH1 0 (in_off)
///   PUSH1 0 (value)   PUSH20 BOB  PUSH2 0xffff (gas) CALL
///   POP
///   PUSH1 7 PUSH1 0 MSTORE                   memory write
///   PUSH1 32 PUSH1 0 RETURN                  return 32 bytes
const CODE: &[u8] = &[
    0x60, 0x2a, 0x60, 0x00, 0x55, // SSTORE
    0x60, 0x00, 0x60, 0x00, 0xa0, // LOG0
    0x60, 0x00, 0x60, 0x00, 0x60, 0x00, 0x60, 0x00, 0x60, 0x00, // call args
    0x73, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb,
    0xbb, 0xbb, 0xbb, 0xbb, 0xbb, // PUSH20 BOB
    0x61, 0xff, 0xff, // PUSH2 0xffff
    0xf1, // CALL
    0x50, // POP
    0x60, 0x07, 0x60, 0x00, 0x52, // MSTORE
    0x60, 0x20, 0x60, 0x00, 0xf3, // RETURN
];

fn eth(amount: u64) -> U256 {
    U256::from(amount) * U256::from(10u64).pow(U256::from(18))
}

// ---------------------------------------------------------------------------
// A trace made only of version-neutral (shared alloy / std) types
// ---------------------------------------------------------------------------

#[derive(Debug, Default, PartialEq)]
struct Trace {
    /// Per step: (pc, opcode, gas_remaining, stack, memory_size).
    steps: Vec<(usize, u8, u64, Vec<U256>, usize)>,
    logs: Vec<revm41::primitives::Log>,
    /// Per `call` callback: (target, caller, gas_limit, is_static).
    calls: Vec<(Address, Address, u64, bool)>,
    /// Per `call_end` callback: (output bytes, gas remaining in outcome).
    call_ends: Vec<(Bytes, u64)>,
    initialized_frames: usize,
}

/// Native tracer: revm@38 `Inspector`.
#[derive(Default)]
struct NativeTracer {
    trace: Trace,
}

impl<ContextT> revm38::Inspector<ContextT> for NativeTracer {
    fn initialize_interp(
        &mut self,
        _interp: &mut revm38::interpreter::Interpreter,
        _context: &mut ContextT,
    ) {
        self.trace.initialized_frames += 1;
    }

    fn step(&mut self, interp: &mut revm38::interpreter::Interpreter, _context: &mut ContextT) {
        use revm38::interpreter::interpreter_types::Jumps as _;
        self.trace.steps.push((
            interp.bytecode.pc(),
            interp.bytecode.opcode(),
            interp.gas.remaining(),
            interp.stack.data().clone(),
            interp.memory.context_memory().len(),
        ));
    }

    fn log(&mut self, _context: &mut ContextT, log: revm41::primitives::Log) {
        self.trace.logs.push(log);
    }

    fn call(
        &mut self,
        _context: &mut ContextT,
        inputs: &mut revm38::interpreter::CallInputs,
    ) -> Option<revm38::interpreter::CallOutcome> {
        self.trace.calls.push((
            inputs.target_address,
            inputs.caller,
            inputs.gas_limit,
            inputs.is_static,
        ));
        None
    }

    fn call_end(
        &mut self,
        _context: &mut ContextT,
        _inputs: &revm38::interpreter::CallInputs,
        outcome: &mut revm38::interpreter::CallOutcome,
    ) {
        self.trace.call_ends.push((
            outcome.result.output.clone(),
            outcome.result.gas.remaining(),
        ));
    }
}

/// The same tracer, written against revm@41's `Inspector` — as EDR's
/// inspectors will be after the upgrade. Driven through `InspectorBridge`.
#[derive(Default)]
struct BridgedTracer {
    trace: Trace,
}

impl revm41::Inspector<()> for BridgedTracer {
    fn initialize_interp(
        &mut self,
        _interp: &mut revm41::interpreter::Interpreter,
        _context: &mut (),
    ) {
        self.trace.initialized_frames += 1;
    }

    fn step(&mut self, interp: &mut revm41::interpreter::Interpreter, _context: &mut ()) {
        use revm41::interpreter::interpreter_types::Jumps as _;
        self.trace.steps.push((
            interp.bytecode.pc(),
            interp.bytecode.opcode(),
            interp.gas.remaining(),
            interp.stack.data().clone(),
            interp.memory.context_memory().len(),
        ));
    }

    fn log(&mut self, _context: &mut (), log: revm41::primitives::Log) {
        self.trace.logs.push(log);
    }

    fn call(
        &mut self,
        _context: &mut (),
        inputs: &mut revm41::interpreter::CallInputs,
    ) -> Option<revm41::interpreter::CallOutcome> {
        self.trace.calls.push((
            inputs.target_address,
            inputs.caller,
            inputs.gas_limit,
            inputs.is_static,
        ));
        None
    }

    fn call_end(
        &mut self,
        _context: &mut (),
        _inputs: &revm41::interpreter::CallInputs,
        outcome: &mut revm41::interpreter::CallOutcome,
    ) {
        self.trace.call_ends.push((
            outcome.result.output.clone(),
            outcome.result.gas.remaining(),
        ));
    }
}

// ---------------------------------------------------------------------------
// Execution scaffolding
// ---------------------------------------------------------------------------

fn seed_old(db: &mut CacheDB38<EmptyDB38>) {
    let code = revm38::bytecode::Bytecode::new_raw(Bytes::from_static(CODE));
    let code_hash = code.hash_slow();
    db.insert_account_info(
        ALICE,
        revm38::state::AccountInfo {
            balance: eth(10),
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            account_id: None,
            code: None,
        },
    );
    db.insert_account_info(
        CONTRACT,
        revm38::state::AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash,
            account_id: None,
            code: Some(code),
        },
    );
}

fn seed_new(db: &mut CacheDB41<EmptyDB41>) {
    let code = revm41::bytecode::Bytecode::new_raw(Bytes::from_static(CODE));
    let code_hash = code.hash_slow();
    db.insert_account_info(
        ALICE,
        revm41::state::AccountInfo {
            balance: eth(10),
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            account_id: None,
            code: None,
        },
    );
    db.insert_account_info(
        CONTRACT,
        revm41::state::AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash,
            account_id: None,
            code: Some(code),
        },
    );
}

fn tx_call_contract() -> OpTransaction<TxEnv38> {
    let base = revm41::context::TxEnv {
        tx_type: 2,
        caller: ALICE,
        gas_limit: 200_000,
        gas_price: 0,
        gas_priority_fee: Some(0),
        kind: TxKind::Call(CONTRACT),
        value: U256::ZERO,
        nonce: 0,
        chain_id: Some(CHAIN_ID),
        ..Default::default()
    };
    OpTransaction {
        base: convert::tx_env_new_to_old(base),
        enveloped_tx: Some(Bytes::from_static(&[0xfa; 8])),
        deposit: Default::default(),
    }
}

fn block_and_cfg() -> (revm38::context::BlockEnv, revm38::context::CfgEnv<OpSpecId>) {
    let block = convert::block_env_new_to_old(revm41::context::BlockEnv {
        number: U256::from(1),
        beneficiary: Address::new([0xfe; 20]),
        timestamp: U256::from(1_700_000_000u64),
        gas_limit: 30_000_000,
        basefee: 0,
        prevrandao: Some(B256::repeat_byte(0x42)),
        ..Default::default()
    });
    let mut cfg = revm41::context::CfgEnv::<OpHardfork>::default();
    cfg.spec = OpHardfork(OpSpecId::ISTHMUS);
    cfg.chain_id = CHAIN_ID;
    (
        block,
        convert::cfg_env_new_to_old(cfg, |hardfork| hardfork.0),
    )
}

/// Runs the contract-call transaction with the given revm@38 inspector over
/// the given revm@38 database; returns the execution result.
fn inspect_tx<DatabaseT, InspectorT>(
    db: DatabaseT,
    inspector: InspectorT,
) -> (
    revm38::context::result::ExecutionResult<op_revm::OpHaltReason>,
    InspectorT,
)
where
    DatabaseT: revm38::Database,
    DatabaseT::Error: core::fmt::Debug,
    InspectorT: for<'any> revm38::Inspector<
        op_revm::OpContext<DatabaseT>,
        revm38::interpreter::interpreter::EthInterpreter,
    >,
{
    let (block, cfg) = block_and_cfg();
    let ctx = Context38::op().with_db(db).with_block(block).with_cfg(cfg);
    let mut evm = ctx.build_op_with_inspector(inspector);
    let output = evm.inspect_tx(tx_call_contract()).expect("tx must execute");
    (output.result, evm.0.inspector)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn bridged_inspector_observes_identical_trace() {
    // Native: revm@38 inspector on revm@38 database.
    let mut db_old = CacheDB38::new(EmptyDB38::default());
    seed_old(&mut db_old);
    let (native_result, native_tracer) = inspect_tx(&mut db_old, NativeTracer::default());

    // Bridged: revm@41 inspector + revm@41 database, through both bridges.
    let mut db_new = CacheDB41::new(EmptyDB41::default());
    seed_new(&mut db_new);
    let (bridged_result, bridge) = inspect_tx(
        DbBridge::new(&mut db_new),
        InspectorBridge::new(BridgedTracer::default()),
    );

    assert!(native_result.is_success(), "{native_result:?}");
    assert_eq!(native_result, bridged_result);

    let native = native_tracer.trace;
    let bridged = bridge.inner.trace;

    assert_eq!(native.initialized_frames, bridged.initialized_frames);
    assert_eq!(native.steps.len(), bridged.steps.len(), "step count");
    for (index, (native_step, bridged_step)) in
        native.steps.iter().zip(bridged.steps.iter()).enumerate()
    {
        assert_eq!(native_step, bridged_step, "step {index} diverged");
    }
    assert_eq!(native.logs, bridged.logs);
    assert_eq!(native.calls, bridged.calls);
    assert_eq!(native.call_ends, bridged.call_ends);

    // The trace must be non-trivial for this test to mean anything. Note:
    // the `call` callback fires for the top-level call too, and a codeless
    // target (BOB) gets no interpreter frame.
    assert!(native.steps.len() > 20, "expected a real trace");
    assert_eq!(native.logs.len(), 1);
    assert_eq!(native.calls.len(), 2, "top-level call + inner CALL");
    assert_eq!(native.calls[0].0, CONTRACT);
    assert_eq!(native.calls[1].0, BOB);
    assert_eq!(
        native.initialized_frames, 1,
        "only the contract frame runs code"
    );
}

/// A gas-burning inspector: charges 1000 extra gas on the first step. The
/// revm@41 version mutates the *mirror*; the write-back must propagate it
/// into the real revm@38 execution.
#[derive(Default)]
struct NativeGasBurner {
    burned: bool,
}

impl<ContextT> revm38::Inspector<ContextT> for NativeGasBurner {
    fn step(&mut self, interp: &mut revm38::interpreter::Interpreter, _context: &mut ContextT) {
        if !self.burned {
            self.burned = true;
            assert!(interp.gas.record_regular_cost(1000));
        }
    }
}

#[derive(Default)]
struct BridgedGasBurner {
    burned: bool,
}

impl revm41::Inspector<()> for BridgedGasBurner {
    fn step(&mut self, interp: &mut revm41::interpreter::Interpreter, _context: &mut ()) {
        if !self.burned {
            self.burned = true;
            assert!(interp.gas.record_regular_cost(1000));
        }
    }
}

#[test]
fn bridged_inspector_mutations_propagate() {
    // Baseline without mutation.
    let mut db_old = CacheDB38::new(EmptyDB38::default());
    seed_old(&mut db_old);
    let (baseline_result, _) = inspect_tx(&mut db_old, NativeTracer::default());

    // Native mutation.
    let mut db_old = CacheDB38::new(EmptyDB38::default());
    seed_old(&mut db_old);
    let (native_result, _) = inspect_tx(&mut db_old, NativeGasBurner::default());

    // Bridged mutation: burned on the mirror, synced back into revm@38.
    let mut db_new = CacheDB41::new(EmptyDB41::default());
    seed_new(&mut db_new);
    let (bridged_result, _) = inspect_tx(
        DbBridge::new(&mut db_new),
        InspectorBridge::new(BridgedGasBurner::default()),
    );

    assert_eq!(
        native_result.tx_gas_used(),
        bridged_result.tx_gas_used(),
        "bridged gas mutation must match native"
    );
    assert_eq!(
        native_result.tx_gas_used(),
        baseline_result.tx_gas_used() + 1000,
        "the burn must actually cost 1000 gas"
    );
}
