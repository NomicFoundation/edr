//! Bridging type for the existing `MessageTrace` interface in Hardhat.

use napi::{
    bindgen_prelude::{BigInt, Either3, Either4, Uint8Array, Undefined},
    Either,
};
use napi_derive::napi;
use serde_json::Value;

use super::exit::Exit;

#[napi(object)]
pub struct EvmStep {
    pub pc: u32,
}

#[napi(object)]
pub struct PrecompileMessageTrace {
    // `BaseMessageTrace`
    pub value: BigInt,
    pub return_data: Uint8Array,
    pub exit: Exit,
    pub gas_used: BigInt,
    pub depth: u32,
    // `PrecompileMessageTrace`
    pub precompile: u32,
    pub calldata: Uint8Array,
}

#[napi(object)]
pub struct CreateMessageTrace {
    // `BaseMessageTrace`
    pub value: BigInt,
    pub return_data: Uint8Array,
    pub exit: Exit,
    pub gas_used: BigInt,
    pub depth: u32,
    // `BaseEvmMessageTrace`
    pub code: Uint8Array,
    pub steps: Vec<Either4<EvmStep, PrecompileMessageTrace, CreateMessageTrace, CallMessageTrace>>,
    // TODO: Will be later filled on the JS side but we should port to ContractsIdentifier in Rust
    // This is explicitly `any` on the JS side to side-step the type-checking until we port
    pub bytecode: Option<Value>,
    pub number_of_subtraces: u32,
    // `CreateMessageTrace`
    // NOTE: we can't use Option<T> as this is converted to an optional property,
    // which is not backwards compatible with the current JS interface
    pub deployed_contract: Either<Uint8Array, Undefined>,
}

#[napi(object)]
pub struct CallMessageTrace {
    // `BaseMessageTrace`
    pub value: BigInt,
    pub return_data: Uint8Array,
    pub exit: Exit,
    pub gas_used: BigInt,
    pub depth: u32,
    // `BaseEvmMessageTrace`
    pub code: Uint8Array,
    pub steps: Vec<Either4<EvmStep, PrecompileMessageTrace, CreateMessageTrace, CallMessageTrace>>,
    // TODO: Will be later filled on the JS side but we should port to ContractsIdentifier in Rust
    // This is explicitly `any` on the JS side to side-step the type-checking until we port
    pub bytecode: Option<Value>,
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
) -> Either4<EvmStep, PrecompileMessageTrace, CreateMessageTrace, CallMessageTrace> {
    match value {
        edr_solidity::message_trace::MessageTraceStep::Evm(step) => {
            Either4::A(EvmStep { pc: step.pc as u32 })
        }
        edr_solidity::message_trace::MessageTraceStep::Message(msg) => {
            // Immediately drop the borrow lock as it may be
            let owned = msg.borrow().clone();
            match message_trace_to_napi(owned) {
                Either3::A(precompile) => Either4::B(precompile),
                Either3::B(create) => Either4::C(create),
                Either3::C(call) => Either4::D(call),
            }
        }
    }
}

/// Converts the Rust representation of a `MessageTrace` to the N-API
/// representation.
pub fn message_trace_to_napi(
    value: edr_solidity::message_trace::MessageTrace,
) -> Either3<PrecompileMessageTrace, CreateMessageTrace, CallMessageTrace> {
    match value {
        edr_solidity::message_trace::MessageTrace::Precompile(precompile) => {
            Either3::A(PrecompileMessageTrace {
                value: BigInt {
                    sign_bit: false,
                    words: precompile.base.value.as_limbs().to_vec(),
                },
                return_data: Uint8Array::from(precompile.base.return_data.as_ref()),
                exit: Exit(precompile.base.exit),
                gas_used: BigInt::from(precompile.base.gas_used),
                depth: precompile.base.depth as u32,

                precompile: precompile.precompile,
                calldata: Uint8Array::from(precompile.calldata.as_ref()),
            })
        }
        edr_solidity::message_trace::MessageTrace::Create(create) => {
            Either3::B(CreateMessageTrace {
                value: BigInt {
                    sign_bit: false,
                    words: create.base.base.value.as_limbs().to_vec(),
                },
                return_data: Uint8Array::from(create.base.base.return_data.as_ref()),
                exit: Exit(create.base.base.exit),
                gas_used: BigInt::from(create.base.base.gas_used),
                depth: create.base.base.depth as u32,
                code: Uint8Array::from(create.base.code.as_ref()),
                steps: create
                    .base
                    .steps
                    .into_iter()
                    .map(message_trace_step_to_napi)
                    .collect(),
                // NOTE: We specifically use None as that will be later filled on the JS side
                bytecode: None,

                number_of_subtraces: create.base.number_of_subtraces,
                deployed_contract: match create.deployed_contract {
                    Some(contract) => Either::A(Uint8Array::from(contract.as_ref())),
                    None => Either::B(()),
                },
            })
        }
        edr_solidity::message_trace::MessageTrace::Call(call) => Either3::C(CallMessageTrace {
            value: BigInt {
                sign_bit: false,
                words: call.base.base.value.as_limbs().to_vec(),
            },
            return_data: Uint8Array::from(call.base.base.return_data.as_ref()),
            exit: Exit(call.base.base.exit),
            gas_used: BigInt::from(call.base.base.gas_used),
            depth: call.base.base.depth as u32,
            code: Uint8Array::from(call.base.code.as_ref()),
            steps: call
                .base
                .steps
                .into_iter()
                .map(message_trace_step_to_napi)
                .collect(),
            // NOTE: We specifically use None as that will be later filled on the JS side
            bytecode: None,
            number_of_subtraces: call.base.number_of_subtraces,

            address: Uint8Array::from(call.address.as_slice()),
            calldata: Uint8Array::from(call.calldata.as_ref()),
            code_address: Uint8Array::from(call.code_address.as_slice()),
        }),
    }
}