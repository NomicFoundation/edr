//! `InspectorBridge`: drive a revm@41 `Inspector` from a revm@38 (op-revm)
//! execution.
//!
//! The hard part is `step`/`step_end`: revm@38 hands the inspector a
//! `&mut Interpreter` (revm@38's type), while the inner revm@41 inspector
//! needs `&mut Interpreter` of revm@41's type. The bridge materializes a
//! revm@41 *mirror* of the interpreter per callback, lets the inner inspector
//! observe/mutate it, and syncs mutations (gas, stack, memory, pc) back.
//!
//! The mirror is cached on the bridge, keyed by the identity of the executing
//! bytecode: a full rebuild (including the O(code) bytecode re-analysis and
//! stack/memory allocations) happens only on frame switches, while steps
//! within a frame refresh contents in place into the existing buffers. The
//! per-step content copies (stack + memory) are inherent to mirroring and are
//! the same cost class as opcode-level tracing itself.
//!
//! Spike limitations (documented, not hidden):
//! - The inner inspector receives `()` as its context: context-reading
//!   inspectors (journal access, cheatcodes) are NOT covered. Bridging the
//!   context would require mirroring the journal — assessed as impractical.
//! - Inspector-forced halts (`Interpreter::halt`) are not forwarded.

use crate::convert;

type Interpreter38 =
    revm38::interpreter::Interpreter<revm38::interpreter::interpreter::EthInterpreter>;
type Interpreter41 =
    revm41::interpreter::Interpreter<revm41::interpreter::interpreter::EthInterpreter>;

type Gas38 = revm38::interpreter::Gas;
type Gas41 = revm41::interpreter::Gas;

type CallInputs38 = revm38::interpreter::CallInputs;
type CallInputs41 = revm41::interpreter::CallInputs;
type CallOutcome38 = revm38::interpreter::CallOutcome;
type CallOutcome41 = revm41::interpreter::CallOutcome;
type CreateInputs38 = revm38::interpreter::CreateInputs;
type CreateInputs41 = revm41::interpreter::CreateInputs;
type CreateOutcome38 = revm38::interpreter::CreateOutcome;
type CreateOutcome41 = revm41::interpreter::CreateOutcome;
type InterpreterResult38 = revm38::interpreter::InterpreterResult;
type InterpreterResult41 = revm41::interpreter::InterpreterResult;
type InstructionResult38 = revm38::interpreter::InstructionResult;
type InstructionResult41 = revm41::interpreter::InstructionResult;
type CallInput38 = revm38::interpreter::CallInput;
type CallInput41 = revm41::interpreter::CallInput;
type CallValue38 = revm38::interpreter::CallValue;
type CallValue41 = revm41::interpreter::CallValue;
type CallScheme38 = revm38::interpreter::CallScheme;
type CallScheme41 = revm41::interpreter::CallScheme;
type CreateScheme38 = revm38::context_interface::CreateScheme;
type CreateScheme41 = revm41::context_interface::CreateScheme;

use revm41::primitives::{Address, Log, U256};

// ---------------------------------------------------------------------------
// The bridge
// ---------------------------------------------------------------------------

pub struct InspectorBridge<InspectorT> {
    pub inner: InspectorT,
    /// Cached interpreter mirror; fully rebuilt only when the executing
    /// bytecode changes (frame switch), refreshed in place otherwise.
    mirror: Option<Mirror>,
}

struct Mirror {
    interp: Interpreter41,
    /// Identity (`ptr`, `len`) of the revm@38 bytecode the mirror was built
    /// from. A different identity means a different frame is executing.
    /// Never dereferenced — comparison only.
    code_identity: (usize, usize),
}

impl<InspectorT> InspectorBridge<InspectorT> {
    pub fn new(inner: InspectorT) -> Self {
        Self {
            inner,
            mirror: None,
        }
    }
}

fn code_identity(interp: &Interpreter38) -> (usize, usize) {
    let code = interp.bytecode.original_byte_slice();
    (code.as_ptr() as usize, code.len())
}

/// Returns the cached mirror, synced to the current revm@38 interpreter
/// state; rebuilds it only if the executing bytecode changed.
///
/// A free function over the `mirror` field (not a method) so callers can
/// borrow `self.inner` at the same time.
fn refreshed_mirror<'mirror>(
    slot: &'mirror mut Option<Mirror>,
    interp: &Interpreter38,
) -> &'mirror mut Interpreter41 {
    let identity = code_identity(interp);
    match slot {
        Some(mirror) if mirror.code_identity == identity => {
            refresh_mirror_in_place(&mut mirror.interp, interp);
        }
        _ => {
            *slot = Some(Mirror {
                interp: interpreter_old_to_new(interp),
                code_identity: identity,
            });
        }
    }
    &mut slot.as_mut().expect("just ensured above").interp
}

impl<ContextT, InspectorT> revm38::Inspector<ContextT> for InspectorBridge<InspectorT>
where
    InspectorT: revm41::Inspector<()>,
{
    fn initialize_interp(&mut self, interp: &mut Interpreter38, _context: &mut ContextT) {
        // New frame: force a rebuild. Within a transaction the journal keeps
        // bytecodes alive, so the (ptr, len) identity cannot be reused by
        // different code — but across transactions it could (ABA); a fresh
        // frame is the boundary where that hazard is closed.
        self.mirror = None;
        let mirror = refreshed_mirror(&mut self.mirror, interp);
        self.inner.initialize_interp(mirror, &mut ());
        sync_interpreter_back(interp, mirror);
    }

    fn step(&mut self, interp: &mut Interpreter38, _context: &mut ContextT) {
        let mirror = refreshed_mirror(&mut self.mirror, interp);
        self.inner.step(mirror, &mut ());
        sync_interpreter_back(interp, mirror);
    }

    fn step_end(&mut self, interp: &mut Interpreter38, _context: &mut ContextT) {
        let mirror = refreshed_mirror(&mut self.mirror, interp);
        self.inner.step_end(mirror, &mut ());
        sync_interpreter_back(interp, mirror);
    }

    fn log(&mut self, _context: &mut ContextT, log: Log) {
        self.inner.log(&mut (), log);
    }

    fn call(
        &mut self,
        _context: &mut ContextT,
        inputs: &mut CallInputs38,
    ) -> Option<CallOutcome38> {
        let mut inputs_new = call_inputs_old_to_new(inputs.clone());
        let outcome = self
            .inner
            .call(&mut (), &mut inputs_new)
            .map(call_outcome_new_to_old);
        // The inspector may rewrite the inputs (e.g. redirect the target).
        *inputs = call_inputs_new_to_old(inputs_new);
        outcome
    }

    fn call_end(
        &mut self,
        _context: &mut ContextT,
        inputs: &CallInputs38,
        outcome: &mut CallOutcome38,
    ) {
        let inputs_new = call_inputs_old_to_new(inputs.clone());
        let mut outcome_new = call_outcome_old_to_new(outcome.clone());
        self.inner.call_end(&mut (), &inputs_new, &mut outcome_new);
        *outcome = call_outcome_new_to_old(outcome_new);
    }

    fn create(
        &mut self,
        _context: &mut ContextT,
        inputs: &mut CreateInputs38,
    ) -> Option<CreateOutcome38> {
        let mut inputs_new = create_inputs_old_to_new(inputs.clone());
        let outcome = self
            .inner
            .create(&mut (), &mut inputs_new)
            .map(create_outcome_new_to_old);
        *inputs = create_inputs_new_to_old(inputs_new);
        outcome
    }

    fn create_end(
        &mut self,
        _context: &mut ContextT,
        inputs: &CreateInputs38,
        outcome: &mut CreateOutcome38,
    ) {
        let inputs_new = create_inputs_old_to_new(inputs.clone());
        let mut outcome_new = create_outcome_old_to_new(outcome.clone());
        self.inner
            .create_end(&mut (), &inputs_new, &mut outcome_new);
        *outcome = create_outcome_new_to_old(outcome_new);
    }

    fn selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
        self.inner.selfdestruct(contract, target, value);
    }
}

// ---------------------------------------------------------------------------
// Interpreter mirror
// ---------------------------------------------------------------------------

fn interpreter_old_to_new(interp: &Interpreter38) -> Interpreter41 {
    use revm38::interpreter::interpreter_types::Jumps as _;
    use revm41::interpreter::interpreter_types::Jumps as _;

    let mut bytecode =
        revm41::interpreter::interpreter::ExtBytecode::new(convert::bytecode_old_to_new(
            // `ExtBytecode` derefs to the underlying `Bytecode`.
            (*interp.bytecode).clone(),
        ));
    bytecode.absolute_jump(interp.bytecode.pc());

    let mut stack = revm41::interpreter::Stack::new();
    for value in interp.stack.data() {
        assert!(stack.push(*value), "mirror stack overflow is impossible");
    }

    let mut memory = revm41::interpreter::SharedMemory::new();
    {
        let context_memory = interp.memory.context_memory();
        memory.resize(context_memory.len());
        memory.set(0, &context_memory);
    }

    let input = inputs_impl_old_to_new(&interp.input);

    Interpreter41 {
        bytecode,
        gas: gas_old_to_new(&interp.gas),
        stack,
        return_data: revm41::interpreter::interpreter::ReturnDataImpl(interp.return_data.0.clone()),
        memory,
        input,
        runtime_flag: revm41::interpreter::interpreter::RuntimeFlags {
            is_static: interp.runtime_flag.is_static,
            spec_id: convert::spec_id_old_to_new(interp.runtime_flag.spec_id),
        },
        extend: (),
    }
}

/// Refreshes an existing mirror (same frame, so same bytecode) from the
/// current revm@38 interpreter state, reusing the stack and memory buffers.
/// `input` is refreshed too: recursion re-enters the same code with
/// different inputs (cheap — `CallInput`/`Bytes` clones are shared-buffer).
fn refresh_mirror_in_place(mirror: &mut Interpreter41, interp: &Interpreter38) {
    use revm38::interpreter::interpreter_types::Jumps as _;
    use revm41::interpreter::interpreter_types::Jumps as _;

    mirror.gas = gas_old_to_new(&interp.gas);

    let stack = mirror.stack.data_mut();
    stack.clear();
    stack.extend_from_slice(interp.stack.data());

    {
        let context_memory = interp.memory.context_memory();
        mirror.memory.resize(context_memory.len());
        mirror.memory.set(0, &context_memory);
    }

    mirror.return_data.0 = interp.return_data.0.clone();
    mirror.input = inputs_impl_old_to_new(&interp.input);
    mirror.runtime_flag = revm41::interpreter::interpreter::RuntimeFlags {
        is_static: interp.runtime_flag.is_static,
        spec_id: convert::spec_id_old_to_new(interp.runtime_flag.spec_id),
    };
    mirror.bytecode.absolute_jump(interp.bytecode.pc());
}

/// Syncs inner-inspector mutations back into the revm@38 interpreter: gas,
/// stack, memory, and program counter. Inspector-forced halts are not
/// forwarded (spike limitation).
fn sync_interpreter_back(interp: &mut Interpreter38, mirror: &Interpreter41) {
    use revm38::interpreter::interpreter_types::Jumps as _;
    use revm41::interpreter::interpreter_types::Jumps as _;

    sync_gas_back(&mut interp.gas, &mirror.gas);

    if interp.stack.data() != mirror.stack.data() {
        let mut stack = revm38::interpreter::Stack::new();
        for value in mirror.stack.data() {
            assert!(stack.push(*value), "mirror stack overflow is impossible");
        }
        interp.stack = stack;
    }

    {
        let mirror_memory = mirror.memory.context_memory();
        let changed = *interp.memory.context_memory() != *mirror_memory;
        if changed {
            interp.memory.resize(mirror_memory.len());
            interp.memory.set(0, &mirror_memory);
        }
    }

    if interp.return_data.0 != mirror.return_data.0 {
        interp.return_data.0 = mirror.return_data.0.clone();
    }

    if interp.bytecode.pc() != mirror.bytecode.pc() {
        interp.bytecode.absolute_jump(mirror.bytecode.pc());
    }
}

/// Mirror construction: full reconstruction through the public API. The
/// internal memory-expansion accounting (`GasTracker`) is not copied — fine
/// for a fresh mirror the inner inspector only reads.
fn gas_old_to_new(gas: &Gas38) -> Gas41 {
    let mut out = Gas41::new_with_regular_gas_and_reservoir(gas.limit(), gas.reservoir());
    out.spend_all();
    out.erase_cost(gas.remaining());
    out.record_refund(gas.refunded());
    // revm@41 tracks state gas as i64 (net of the EIP-7702 refund); revm@38 as u64.
    out.set_state_gas_spent(
        i64::try_from(gas.state_gas_spent()).expect("state gas exceeds i64 range"),
    );
    out
}

fn gas_new_to_old(gas: &Gas41) -> Gas38 {
    let mut out = Gas38::new_with_regular_gas_and_reservoir(gas.limit(), gas.reservoir());
    out.spend_all();
    out.erase_cost(gas.remaining());
    out.record_refund(gas.refunded());
    out.set_state_gas_spent(state_gas_new_to_old(gas.state_gas_spent()));
    out
}

fn state_gas_new_to_old(state_gas: i64) -> u64 {
    u64::try_from(state_gas).expect("negative state gas has no revm@38 representation")
}

/// Write-back applies *deltas* to the original instead of reconstructing it,
/// preserving revm@38's internal memory-expansion accounting.
fn sync_gas_back(gas: &mut Gas38, mirror: &Gas41) {
    assert_eq!(
        gas.limit(),
        mirror.limit(),
        "inspectors cannot change the gas limit"
    );
    if mirror.remaining() > gas.remaining() {
        gas.erase_cost(mirror.remaining() - gas.remaining());
    } else if mirror.remaining() < gas.remaining() {
        let charged = gas.record_regular_cost(gas.remaining() - mirror.remaining());
        assert!(charged, "delta cannot exceed remaining gas");
    }
    if mirror.refunded() != gas.refunded() {
        gas.record_refund(mirror.refunded() - gas.refunded());
    }
    gas.set_state_gas_spent(state_gas_new_to_old(mirror.state_gas_spent()));
    gas.set_reservoir(mirror.reservoir());
}

fn inputs_impl_old_to_new(
    input: &revm38::interpreter::interpreter::InputsImpl,
) -> revm41::interpreter::interpreter::InputsImpl {
    let revm38::interpreter::interpreter::InputsImpl {
        target_address,
        bytecode_address,
        caller_address,
        input,
        call_value,
    } = input;

    revm41::interpreter::interpreter::InputsImpl {
        target_address: *target_address,
        bytecode_address: *bytecode_address,
        caller_address: *caller_address,
        input: call_input_old_to_new(input.clone()),
        call_value: *call_value,
    }
}

// ---------------------------------------------------------------------------
// Call/create inputs and outcomes (both directions: `&mut` args round-trip)
// ---------------------------------------------------------------------------

fn call_input_old_to_new(input: CallInput38) -> CallInput41 {
    match input {
        CallInput38::Bytes(bytes) => CallInput41::Bytes(bytes),
        CallInput38::SharedBuffer(range) => CallInput41::SharedBuffer(range),
    }
}

fn call_input_new_to_old(input: CallInput41) -> CallInput38 {
    match input {
        CallInput41::Bytes(bytes) => CallInput38::Bytes(bytes),
        CallInput41::SharedBuffer(range) => CallInput38::SharedBuffer(range),
    }
}

fn call_value_old_to_new(value: CallValue38) -> CallValue41 {
    match value {
        CallValue38::Transfer(value) => CallValue41::Transfer(value),
        CallValue38::Apparent(value) => CallValue41::Apparent(value),
    }
}

fn call_value_new_to_old(value: CallValue41) -> CallValue38 {
    match value {
        CallValue41::Transfer(value) => CallValue38::Transfer(value),
        CallValue41::Apparent(value) => CallValue38::Apparent(value),
    }
}

fn call_scheme_old_to_new(scheme: CallScheme38) -> CallScheme41 {
    match scheme {
        CallScheme38::Call => CallScheme41::Call,
        CallScheme38::CallCode => CallScheme41::CallCode,
        CallScheme38::DelegateCall => CallScheme41::DelegateCall,
        CallScheme38::StaticCall => CallScheme41::StaticCall,
    }
}

fn call_scheme_new_to_old(scheme: CallScheme41) -> CallScheme38 {
    match scheme {
        CallScheme41::Call => CallScheme38::Call,
        CallScheme41::CallCode => CallScheme38::CallCode,
        CallScheme41::DelegateCall => CallScheme38::DelegateCall,
        CallScheme41::StaticCall => CallScheme38::StaticCall,
    }
}

fn call_inputs_old_to_new(inputs: CallInputs38) -> CallInputs41 {
    let CallInputs38 {
        input,
        return_memory_offset,
        gas_limit,
        reservoir,
        bytecode_address,
        known_bytecode,
        target_address,
        caller,
        value,
        scheme,
        is_static,
    } = inputs;
    let (code_hash, code) = known_bytecode;

    CallInputs41 {
        input: call_input_old_to_new(input),
        return_memory_offset,
        gas_limit,
        reservoir,
        bytecode_address,
        known_bytecode: (code_hash, convert::bytecode_old_to_new(code)),
        target_address,
        caller,
        value: call_value_old_to_new(value),
        scheme: call_scheme_old_to_new(scheme),
        is_static,
        // EIP-8037 bookkeeping; never charged on the pre-8037 revm@38 side.
        charged_new_account_state_gas: false,
    }
}

fn call_inputs_new_to_old(inputs: CallInputs41) -> CallInputs38 {
    let CallInputs41 {
        input,
        return_memory_offset,
        gas_limit,
        reservoir,
        bytecode_address,
        known_bytecode,
        target_address,
        caller,
        value,
        scheme,
        is_static,
        charged_new_account_state_gas,
    } = inputs;
    let (code_hash, code) = known_bytecode;

    assert!(
        !charged_new_account_state_gas,
        "charged state gas has no revm@38 representation"
    );

    CallInputs38 {
        input: call_input_new_to_old(input),
        return_memory_offset,
        gas_limit,
        reservoir,
        bytecode_address,
        known_bytecode: (code_hash, convert::bytecode_new_to_old(code)),
        target_address,
        caller,
        value: call_value_new_to_old(value),
        scheme: call_scheme_new_to_old(scheme),
        is_static,
    }
}

fn create_scheme_old_to_new(scheme: CreateScheme38) -> CreateScheme41 {
    match scheme {
        CreateScheme38::Create => CreateScheme41::Create,
        CreateScheme38::Create2 { salt } => CreateScheme41::Create2 { salt },
        CreateScheme38::Custom { address } => CreateScheme41::Custom { address },
    }
}

fn create_scheme_new_to_old(scheme: CreateScheme41) -> CreateScheme38 {
    match scheme {
        CreateScheme41::Create => CreateScheme38::Create,
        CreateScheme41::Create2 { salt } => CreateScheme38::Create2 { salt },
        CreateScheme41::Custom { address } => CreateScheme38::Custom { address },
    }
}

// `CreateInputs` has private cache fields on both sides; converted through
// the accessor/constructor API instead of destructuring (caches are rebuilt
// lazily on the target side).

fn create_inputs_old_to_new(inputs: CreateInputs38) -> CreateInputs41 {
    CreateInputs41::new(
        inputs.caller(),
        create_scheme_old_to_new(inputs.scheme()),
        inputs.value(),
        inputs.init_code().clone(),
        inputs.gas_limit(),
        inputs.reservoir(),
    )
}

fn create_inputs_new_to_old(inputs: CreateInputs41) -> CreateInputs38 {
    CreateInputs38::new(
        inputs.caller(),
        create_scheme_new_to_old(inputs.scheme()),
        inputs.value(),
        inputs.init_code().clone(),
        inputs.gas_limit(),
        inputs.reservoir(),
    )
}

fn interpreter_result_old_to_new(result: InterpreterResult38) -> InterpreterResult41 {
    let InterpreterResult38 {
        result,
        output,
        gas,
    } = result;

    InterpreterResult41 {
        result: instruction_result_old_to_new(result),
        output,
        gas: gas_old_to_new(&gas),
    }
}

fn interpreter_result_new_to_old(result: InterpreterResult41) -> InterpreterResult38 {
    let InterpreterResult41 {
        result,
        output,
        gas,
    } = result;

    InterpreterResult38 {
        result: instruction_result_new_to_old(result),
        output,
        gas: gas_new_to_old(&gas),
    }
}

fn call_outcome_old_to_new(outcome: CallOutcome38) -> CallOutcome41 {
    let CallOutcome38 {
        result,
        memory_offset,
        was_precompile_called,
        precompile_call_logs,
    } = outcome;

    CallOutcome41 {
        result: interpreter_result_old_to_new(result),
        memory_offset,
        was_precompile_called,
        precompile_call_logs,
        // EIP-8037 bookkeeping; never charged on the pre-8037 revm@38 side.
        charged_new_account_state_gas: false,
    }
}

fn call_outcome_new_to_old(outcome: CallOutcome41) -> CallOutcome38 {
    let CallOutcome41 {
        result,
        memory_offset,
        was_precompile_called,
        precompile_call_logs,
        charged_new_account_state_gas,
    } = outcome;

    assert!(
        !charged_new_account_state_gas,
        "charged state gas has no revm@38 representation"
    );

    CallOutcome38 {
        result: interpreter_result_new_to_old(result),
        memory_offset,
        was_precompile_called,
        precompile_call_logs,
    }
}

fn create_outcome_old_to_new(outcome: CreateOutcome38) -> CreateOutcome41 {
    let CreateOutcome38 { result, address } = outcome;

    CreateOutcome41 {
        result: interpreter_result_old_to_new(result),
        address,
    }
}

fn create_outcome_new_to_old(outcome: CreateOutcome41) -> CreateOutcome38 {
    let CreateOutcome41 { result, address } = outcome;

    CreateOutcome38 {
        result: interpreter_result_new_to_old(result),
        address,
    }
}

// ---------------------------------------------------------------------------
// InstructionResult
// ---------------------------------------------------------------------------

fn instruction_result_old_to_new(result: InstructionResult38) -> InstructionResult41 {
    match result {
        InstructionResult38::Stop => InstructionResult41::Stop,
        InstructionResult38::Return => InstructionResult41::Return,
        InstructionResult38::SelfDestruct => InstructionResult41::SelfDestruct,
        InstructionResult38::Revert => InstructionResult41::Revert,
        InstructionResult38::CallTooDeep => InstructionResult41::CallTooDeep,
        InstructionResult38::OutOfFunds => InstructionResult41::OutOfFunds,
        InstructionResult38::CreateInitCodeStartingEF00 => {
            InstructionResult41::CreateInitCodeStartingEF00
        }
        InstructionResult38::InvalidEOFInitCode => InstructionResult41::InvalidEOFInitCode,
        InstructionResult38::InvalidExtDelegateCallTarget => {
            InstructionResult41::InvalidExtDelegateCallTarget
        }
        InstructionResult38::OutOfGas => InstructionResult41::OutOfGas,
        InstructionResult38::MemoryOOG => InstructionResult41::MemoryOOG,
        InstructionResult38::MemoryLimitOOG => InstructionResult41::MemoryLimitOOG,
        InstructionResult38::PrecompileOOG => InstructionResult41::PrecompileOOG,
        InstructionResult38::InvalidOperandOOG => InstructionResult41::InvalidOperandOOG,
        InstructionResult38::ReentrancySentryOOG => InstructionResult41::ReentrancySentryOOG,
        InstructionResult38::OpcodeNotFound => InstructionResult41::OpcodeNotFound,
        InstructionResult38::CallNotAllowedInsideStatic => {
            InstructionResult41::CallNotAllowedInsideStatic
        }
        InstructionResult38::StateChangeDuringStaticCall => {
            InstructionResult41::StateChangeDuringStaticCall
        }
        InstructionResult38::InvalidFEOpcode => InstructionResult41::InvalidFEOpcode,
        InstructionResult38::InvalidJump => InstructionResult41::InvalidJump,
        InstructionResult38::NotActivated => InstructionResult41::NotActivated,
        InstructionResult38::StackUnderflow => InstructionResult41::StackUnderflow,
        InstructionResult38::StackOverflow => InstructionResult41::StackOverflow,
        InstructionResult38::OutOfOffset => InstructionResult41::OutOfOffset,
        InstructionResult38::CreateCollision => InstructionResult41::CreateCollision,
        InstructionResult38::OverflowPayment => InstructionResult41::OverflowPayment,
        InstructionResult38::PrecompileError => InstructionResult41::PrecompileError,
        InstructionResult38::NonceOverflow => InstructionResult41::NonceOverflow,
        InstructionResult38::CreateContractSizeLimit => {
            InstructionResult41::CreateContractSizeLimit
        }
        InstructionResult38::CreateContractStartingWithEF => {
            InstructionResult41::CreateContractStartingWithEF
        }
        InstructionResult38::CreateInitCodeSizeLimit => {
            InstructionResult41::CreateInitCodeSizeLimit
        }
        InstructionResult38::FatalExternalError => InstructionResult41::FatalExternalError,
        InstructionResult38::InvalidImmediateEncoding => {
            InstructionResult41::InvalidImmediateEncoding
        }
    }
}

fn instruction_result_new_to_old(result: InstructionResult41) -> InstructionResult38 {
    match result {
        InstructionResult41::Stop => InstructionResult38::Stop,
        // revm@41-only internal variant; must never surface to an inspector
        // outcome that crosses the boundary.
        InstructionResult41::Suspend => {
            panic!("`Suspend` has no revm@38 representation")
        }
        InstructionResult41::Return => InstructionResult38::Return,
        InstructionResult41::SelfDestruct => InstructionResult38::SelfDestruct,
        InstructionResult41::Revert => InstructionResult38::Revert,
        InstructionResult41::CallTooDeep => InstructionResult38::CallTooDeep,
        InstructionResult41::OutOfFunds => InstructionResult38::OutOfFunds,
        InstructionResult41::CreateInitCodeStartingEF00 => {
            InstructionResult38::CreateInitCodeStartingEF00
        }
        InstructionResult41::InvalidEOFInitCode => InstructionResult38::InvalidEOFInitCode,
        InstructionResult41::InvalidExtDelegateCallTarget => {
            InstructionResult38::InvalidExtDelegateCallTarget
        }
        InstructionResult41::OutOfGas => InstructionResult38::OutOfGas,
        InstructionResult41::MemoryOOG => InstructionResult38::MemoryOOG,
        InstructionResult41::MemoryLimitOOG => InstructionResult38::MemoryLimitOOG,
        InstructionResult41::PrecompileOOG => InstructionResult38::PrecompileOOG,
        InstructionResult41::InvalidOperandOOG => InstructionResult38::InvalidOperandOOG,
        InstructionResult41::ReentrancySentryOOG => InstructionResult38::ReentrancySentryOOG,
        InstructionResult41::OpcodeNotFound => InstructionResult38::OpcodeNotFound,
        InstructionResult41::CallNotAllowedInsideStatic => {
            InstructionResult38::CallNotAllowedInsideStatic
        }
        InstructionResult41::StateChangeDuringStaticCall => {
            InstructionResult38::StateChangeDuringStaticCall
        }
        InstructionResult41::InvalidFEOpcode => InstructionResult38::InvalidFEOpcode,
        InstructionResult41::InvalidJump => InstructionResult38::InvalidJump,
        InstructionResult41::NotActivated => InstructionResult38::NotActivated,
        InstructionResult41::StackUnderflow => InstructionResult38::StackUnderflow,
        InstructionResult41::StackOverflow => InstructionResult38::StackOverflow,
        InstructionResult41::OutOfOffset => InstructionResult38::OutOfOffset,
        InstructionResult41::CreateCollision => InstructionResult38::CreateCollision,
        InstructionResult41::OverflowPayment => InstructionResult38::OverflowPayment,
        InstructionResult41::PrecompileError => InstructionResult38::PrecompileError,
        InstructionResult41::NonceOverflow => InstructionResult38::NonceOverflow,
        InstructionResult41::CreateContractSizeLimit => {
            InstructionResult38::CreateContractSizeLimit
        }
        InstructionResult41::CreateContractStartingWithEF => {
            InstructionResult38::CreateContractStartingWithEF
        }
        InstructionResult41::CreateInitCodeSizeLimit => {
            InstructionResult38::CreateInitCodeSizeLimit
        }
        InstructionResult41::FatalExternalError => InstructionResult38::FatalExternalError,
        InstructionResult41::InvalidImmediateEncoding => {
            InstructionResult38::InvalidImmediateEncoding
        }
    }
}
