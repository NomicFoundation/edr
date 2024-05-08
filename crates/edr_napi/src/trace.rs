use std::sync::Arc;

use edr_evm::{interpreter::OPCODE_JUMPMAP, trace::BeforeMessage};
use napi::{
    bindgen_prelude::{BigInt, Buffer, Either3},
    Env, JsBuffer, JsBufferValue,
};
use napi_derive::napi;

use crate::result::ExecutionResult;

#[napi(object)]
pub struct TracingMessage {
    /// Sender address
    #[napi(readonly)]
    pub caller: Buffer,

    /// Recipient address. None if it is a Create message.
    #[napi(readonly)]
    pub to: Option<Buffer>,

    /// Whether it's a static call
    #[napi(readonly)]
    pub is_static_call: bool,

    /// Transaction gas limit
    #[napi(readonly)]
    pub gas_limit: BigInt,

    /// Depth of the message
    #[napi(readonly)]
    pub depth: u8,

    /// Input data of the message
    #[napi(readonly)]
    pub data: JsBuffer,

    /// Value sent in the message
    #[napi(readonly)]
    pub value: BigInt,

    /// Address of the code that is being executed. Can be different from `to`
    /// if a delegate call is being done.
    #[napi(readonly)]
    pub code_address: Option<Buffer>,

    /// Code of the contract that is being executed.
    #[napi(readonly)]
    pub code: Option<JsBuffer>,
}

impl TracingMessage {
    pub fn new(env: &Env, message: &BeforeMessage) -> napi::Result<Self> {
        // Deconstruct to make sure all fields are handled
        let BeforeMessage {
            depth,
            caller,
            to: _,
            is_static_call,
            gas_limit,
            data,
            value,
            code_address,
            code,
        } = message;

        let data = env
            .create_buffer_with_data(data.to_vec())
            .map(JsBufferValue::into_raw)?;

        let code = code.as_ref().map_or(Ok(None), |code| {
            env.create_buffer_with_data(code.original_bytes().to_vec())
                .map(JsBufferValue::into_raw)
                .map(Some)
        })?;

        Ok(TracingMessage {
            caller: Buffer::from(caller.as_slice()),
            to: message.to.map(|to| Buffer::from(to.as_slice())),
            gas_limit: BigInt::from(*gas_limit),
            is_static_call: *is_static_call,
            depth: *depth as u8,
            data,
            value: BigInt {
                sign_bit: false,
                words: value.into_limbs().to_vec(),
            },
            code_address: code_address.map(|address| Buffer::from(address.to_vec())),
            code,
        })
    }
}

#[napi(object)]
pub struct TracingStep {
    /// Call depth
    #[napi(readonly)]
    pub depth: u8,
    /// The program counter
    #[napi(readonly)]
    pub pc: BigInt,
    /// The executed op code
    #[napi(readonly)]
    pub opcode: String,
    /// The top entry on the stack. None if the stack is empty.
    #[napi(readonly)]
    pub stack_top: Option<BigInt>,
    /// The entries on the stack. None if verbose tracing is disabled.
    #[napi(readonly)]
    pub stack: Option<Vec<BigInt>>,
    /// The memory at the step. None if verbose tracing is disabled.
    #[napi(readonly)]
    pub memory: Option<Buffer>,
}

impl TracingStep {
    pub fn new(step: &edr_evm::trace::Step) -> Self {
        let stack_top = step.stack.top().map(u256_to_bigint);
        let stack = step
            .stack
            .full()
            .map(|stack| stack.iter().map(u256_to_bigint).collect());
        let memory = step.memory.as_ref().cloned().map(Buffer::from);

        Self {
            depth: step.depth as u8,
            pc: BigInt::from(step.pc),
            opcode: OPCODE_JUMPMAP[usize::from(step.opcode)]
                .unwrap_or("")
                .to_string(),
            stack_top,
            stack,
            memory,
        }
    }
}

fn u256_to_bigint(v: &edr_evm::U256) -> BigInt {
    BigInt {
        sign_bit: false,
        words: v.into_limbs().to_vec(),
    }
}

#[napi(object)]
pub struct TracingMessageResult {
    /// Execution result
    #[napi(readonly)]
    pub execution_result: ExecutionResult,
}

#[napi]
pub struct RawTrace {
    inner: Arc<edr_evm::trace::Trace>,
}

impl RawTrace {
    pub fn new(inner: Arc<edr_evm::trace::Trace>) -> Self {
        Self { inner }
    }
}

#[napi]
impl RawTrace {
    #[napi]
    pub fn trace(
        &self,
        env: Env,
    ) -> napi::Result<Vec<Either3<TracingMessage, TracingStep, TracingMessageResult>>> {
        self.inner
            .messages
            .iter()
            .map(|message| match message {
                edr_evm::trace::TraceMessage::Before(message) => {
                    TracingMessage::new(&env, message).map(Either3::A)
                }
                edr_evm::trace::TraceMessage::Step(step) => Ok(Either3::B(TracingStep::new(step))),
                edr_evm::trace::TraceMessage::After {
                    result,
                    contract_address: address,
                    contract_code: code,
                } => ExecutionResult::new(&env, result, address, code)
                    .map(|execution_result| Either3::C(TracingMessageResult { execution_result })),
            })
            .collect::<napi::Result<_>>()
    }
}
