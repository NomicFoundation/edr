//! Bridging type for the existing `MessageTrace` interface in Hardhat.

use napi::{
    bindgen_prelude::{BigInt, ClassInstance, Either3, Either4, Uint8Array, Undefined},
    Either, Env,
};
use napi_derive::napi;

use super::{exit::Exit, model::BytecodeWrapper};

#[napi(object)]
pub struct EvmStep {
    pub pc: u32,
}

#[napi(object)]
pub struct PrecompileMessageTrace {
    // `BaseMessageTrace`
    pub value: BigInt,
    pub return_data: Uint8Array,
    pub exit: ClassInstance<Exit>,
    pub gas_used: BigInt,
    pub depth: u32,
    // `PrecompileMessageTrace`
    pub precompile: u32,
    pub calldata: Uint8Array,
}

// NOTE: Because of the hack below for `deployed_contract`, now the
// `CallMessageTrace` is a strict superset of `CreateMessageTrace`, so we need
// to take care to keep the order consistent from most-specific to
// least-specific in the `Either{3,4}` type when converting to or from N-API.
#[napi(object)]
pub struct CreateMessageTrace {
    // `BaseMessageTrace`
    pub value: BigInt,
    pub return_data: Uint8Array,
    pub exit: ClassInstance<Exit>,
    pub gas_used: BigInt,
    pub depth: u32,
    // `BaseEvmMessageTrace`
    pub code: Uint8Array,
    pub steps: Vec<Either4<EvmStep, PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace>>,
    /// Reference to the resolved `Bytecode` EDR data.
    /// Only used on the JS side by the `VmTraceDecoder` class.
    pub bytecode: Option<ClassInstance<BytecodeWrapper>>,
    pub number_of_subtraces: u32,
    // `CreateMessageTrace`
    // HACK: It seems that `Either<Uint8Array, Undefined>` means exactly what we
    // want (a required property but can be explicitly `undefined`) but internally
    // the napi-rs treats an encountered `Undefined` like a missing property
    // and it throws a validation error. While not 100% backwards compatible, we
    // work around using an optional type.
    // See https://github.com/napi-rs/napi-rs/issues/1986 for context on the PR
    // that introduced this behavior.
    pub deployed_contract: Option<Either<Uint8Array, Undefined>>,
}

#[napi(object)]
pub struct CallMessageTrace {
    // `BaseMessageTrace`
    pub value: BigInt,
    pub return_data: Uint8Array,
    pub exit: ClassInstance<Exit>,
    pub gas_used: BigInt,
    pub depth: u32,
    // `BaseEvmMessageTrace`
    pub code: Uint8Array,
    pub steps: Vec<Either4<EvmStep, PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace>>,
    /// Reference to the resolved `Bytecode` EDR data.
    /// Only used on the JS side by the `VmTraceDecoder` class.
    pub bytecode: Option<ClassInstance<BytecodeWrapper>>,
    pub number_of_subtraces: u32,
    // `CallMessageTrace`
    pub calldata: Uint8Array,
    pub address: Uint8Array,
    pub code_address: Uint8Array,
}

/// Converts [`edr_solidity::message_trace::MessageTraceStep`] to the N-API
/// representation.
///
/// # Panics
/// This function will panic if the value is mutably borrowed.
pub fn message_trace_step_to_napi(
    value: edr_solidity::message_trace::MessageTraceStep,
    env: Env,
) -> napi::Result<Either4<EvmStep, PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace>> {
    Ok(match value {
        edr_solidity::message_trace::MessageTraceStep::Evm(step) => {
            Either4::A(EvmStep { pc: step.pc as u32 })
        }
        edr_solidity::message_trace::MessageTraceStep::Message(msg) => {
            // Immediately drop the borrow lock to err on the safe side as we
            // may be recursing.
            let owned = msg.borrow().clone();
            match message_trace_to_napi(owned, env)? {
                Either3::A(precompile) => Either4::B(precompile),
                Either3::B(call) => Either4::C(call),
                Either3::C(create) => Either4::D(create),
            }
        }
    })
}

/// Converts the Rust representation of a `MessageTrace` to the N-API
/// representation.
pub fn message_trace_to_napi(
    value: edr_solidity::message_trace::MessageTrace,
    env: Env,
) -> napi::Result<Either3<PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace>> {
    Ok(match value {
        edr_solidity::message_trace::MessageTrace::Precompile(precompile) => {
            Either3::A(PrecompileMessageTrace {
                value: BigInt {
                    sign_bit: false,
                    words: precompile.base.value.as_limbs().to_vec(),
                },
                return_data: Uint8Array::from(precompile.base.return_data.as_ref()),
                exit: Exit(precompile.base.exit.into()).into_instance(env)?,
                gas_used: BigInt::from(precompile.base.gas_used),
                depth: precompile.base.depth as u32,

                precompile: precompile.precompile,
                calldata: Uint8Array::from(precompile.calldata.as_ref()),
            })
        }
        edr_solidity::message_trace::MessageTrace::Call(call) => Either3::B(CallMessageTrace {
            value: BigInt {
                sign_bit: false,
                words: call.base.base.value.as_limbs().to_vec(),
            },
            return_data: Uint8Array::from(call.base.base.return_data.as_ref()),
            exit: Exit(call.base.base.exit.into()).into_instance(env)?,
            gas_used: BigInt::from(call.base.base.gas_used),
            depth: call.base.base.depth as u32,
            code: Uint8Array::from(call.base.code.as_ref()),
            steps: call
                .base
                .steps
                .into_iter()
                .map(|step| message_trace_step_to_napi(step, env))
                .collect::<napi::Result<Vec<_>>>()?,
            // NOTE: We specifically use None as that will be later filled on the JS side
            bytecode: None,
            number_of_subtraces: call.base.number_of_subtraces,

            address: Uint8Array::from(call.address.as_slice()),
            calldata: Uint8Array::from(call.calldata.as_ref()),
            code_address: Uint8Array::from(call.code_address.as_slice()),
        }),
        edr_solidity::message_trace::MessageTrace::Create(create) => {
            Either3::C(CreateMessageTrace {
                value: BigInt {
                    sign_bit: false,
                    words: create.base.base.value.as_limbs().to_vec(),
                },
                return_data: Uint8Array::from(create.base.base.return_data.as_ref()),
                exit: Exit(create.base.base.exit.into()).into_instance(env)?,
                gas_used: BigInt::from(create.base.base.gas_used),
                depth: create.base.base.depth as u32,
                code: Uint8Array::from(create.base.code.as_ref()),
                steps: create
                    .base
                    .steps
                    .into_iter()
                    .map(|step| message_trace_step_to_napi(step, env))
                    .collect::<napi::Result<Vec<_>>>()?,
                // NOTE: We specifically use None as that will be later filled on the JS side
                bytecode: None,

                number_of_subtraces: create.base.number_of_subtraces,
                deployed_contract: create
                    .deployed_contract
                    .map(|contract| Either::A(Uint8Array::from(contract.as_ref()))),
            })
        }
    })
}
