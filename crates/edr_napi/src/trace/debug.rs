//! Port of `hardhat-network/stack-traces/debug.ts` from Hardhat.

use edr_eth::U256;
use edr_evm::{hex, interpreter::OpCode};
use edr_solidity::build_model::JumpType;
use napi::{
    bindgen_prelude::{Either24, Either3, Either4},
    Either, Env,
};
use napi_derive::napi;

use super::{
    message_trace::{CallMessageTrace, CreateMessageTrace, PrecompileMessageTrace},
    solidity_stack_trace::{RevertErrorStackTraceEntry, SolidityStackTrace},
};
use crate::trace::return_data::ReturnData;

const MARGIN_SPACE: usize = 6;

#[napi]
fn print_message_trace(
    trace: Either3<PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace>,
    depth: Option<u32>,
    env: Env,
) -> napi::Result<()> {
    let trace = match &trace {
        Either3::A(precompile) => Either3::A(precompile),
        Either3::B(call) => Either3::B(call),
        Either3::C(create) => Either3::C(create),
    };

    let depth = depth.unwrap_or(0);

    print_message_trace_inner(trace, depth, env)
}

fn print_message_trace_inner(
    trace: Either3<&PrecompileMessageTrace, &CallMessageTrace, &CreateMessageTrace>,
    depth: u32,
    env: Env,
) -> napi::Result<()> {
    println!();

    match trace {
        Either3::A(precompile) => print_precompile_trace(precompile, depth),
        Either3::B(call) => print_call_trace(call, depth, env)?,
        Either3::C(create) => print_create_trace(create, depth, env)?,
    }

    println!();

    Ok(())
}

fn print_precompile_trace(trace: &PrecompileMessageTrace, depth: u32) {
    let margin = " ".repeat(depth as usize * MARGIN_SPACE);

    let value = U256::from_limbs_slice(&trace.value.words);

    println!("{margin}Precompile trace");

    println!("{margin} precompile number: {}", trace.precompile);
    println!("{margin} value: {value}");
    println!(
        "{margin} calldata: {}",
        hex::encode_prefixed(&*trace.calldata)
    );

    if trace.exit.is_error() {
        println!("{margin} error: {}", trace.exit.get_reason());
    }

    println!(
        "{margin} returnData: {}",
        hex::encode_prefixed(&*trace.return_data)
    );
}

fn print_call_trace(trace: &CallMessageTrace, depth: u32, env: Env) -> napi::Result<()> {
    let margin = " ".repeat(depth as usize * MARGIN_SPACE);

    println!("{margin}Call trace");

    if let Some(bytecode) = &trace.bytecode {
        let contract = bytecode.contract.borrow();
        let file = contract.location.file();
        let file = file.borrow();

        println!(
            "{margin} calling contract: {}:{}",
            file.source_name, contract.name
        );
    } else {
        println!(
            "{margin} unrecognized contract code: {:?}",
            hex::encode_prefixed(&*trace.code)
        );
        println!(
            "{margin} contract: {}",
            hex::encode_prefixed(&*trace.address)
        );
    }

    println!(
        "{margin} value: {}",
        U256::from_limbs_slice(&trace.value.words)
    );
    println!(
        "{margin} calldata: {}",
        hex::encode_prefixed(&*trace.calldata)
    );

    if trace.exit.is_error() {
        println!("{margin} error: {}", trace.exit.get_reason());
    }

    println!(
        "{margin} returnData: {}",
        hex::encode_prefixed(&*trace.return_data)
    );

    trace_steps(Either::A(trace), depth, env)
}

fn print_create_trace(trace: &CreateMessageTrace, depth: u32, env: Env) -> napi::Result<()> {
    let margin = " ".repeat(depth as usize * MARGIN_SPACE);

    println!("{margin}Create trace");

    if let Some(bytecode) = &trace.bytecode {
        let contract = bytecode.contract.borrow();

        println!("{margin} deploying contract: {}", contract.name);
        println!("{margin} code: {}", hex::encode_prefixed(&*trace.code));
    } else {
        println!(
            "{margin} unrecognized deployment code: {}",
            hex::encode_prefixed(&*trace.code)
        );
    }

    println!(
        "{margin} value: {}",
        U256::from_limbs_slice(&trace.value.words)
    );

    if let Some(Either::A(deployed_contract)) = &trace.deployed_contract {
        println!(
            "{margin} contract address: {}",
            hex::encode_prefixed(deployed_contract)
        );
    }

    if trace.exit.is_error() {
        println!("{margin} error: {}", trace.exit.get_reason());
        // The return data is the deployed-bytecode if there was no error, so we don't
        // show it
        println!(
            "{margin} returnData: {}",
            hex::encode_prefixed(&*trace.return_data)
        );
    }

    trace_steps(Either::B(trace), depth, env)?;

    Ok(())
}

fn trace_steps(
    trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    depth: u32,
    env: Env,
) -> napi::Result<()> {
    let margin = " ".repeat(depth as usize * MARGIN_SPACE);

    println!("{margin} steps:");
    println!();

    let (bytecode, steps) = match &trace {
        Either::A(call) => (&call.bytecode, &call.steps),
        Either::B(create) => (&create.bytecode, &create.steps),
    };

    for step in steps {
        let step = match step {
            Either4::A(step) => step,
            trace @ (Either4::B(..) | Either4::C(..) | Either4::D(..)) => {
                let trace = match trace {
                    Either4::A(..) => unreachable!(),
                    Either4::B(precompile) => Either3::A(precompile),
                    Either4::C(call) => Either3::B(call),
                    Either4::D(create) => Either3::C(create),
                };

                print_message_trace_inner(trace, depth + 1, env)?;
                continue;
            }
        };

        let pc = format!("{:>5}", format!("{:03}", step.pc));

        if let Some(bytecode) = bytecode {
            let inst = bytecode.get_instruction(step.pc)?;

            let location = inst
                .location
                .as_ref()
                .map(|inst_location| {
                    let inst_location = &inst_location;
                    let file = inst_location.file();
                    let file = file.borrow();

                    let mut location_str = file.source_name.clone();

                    if let Some(func) = inst_location.get_containing_function() {
                        let file = func.location.file();
                        let file = file.borrow();

                        let source_name = func
                            .contract_name
                            .as_ref()
                            .unwrap_or_else(|| &file.source_name);

                        location_str += &format!(":{source_name}:{}", func.name);
                    }
                    location_str +=
                        &format!("   -  {}:{}", inst_location.offset, inst_location.length);

                    napi::Result::Ok(location_str)
                })
                .transpose()?
                .unwrap_or_default();

            if matches!(inst.opcode, OpCode::JUMP | OpCode::JUMPI) {
                let jump = if inst.jump_type == JumpType::NotJump {
                    "".to_string()
                } else {
                    format!("({})", inst.jump_type)
                };

                let entry = format!("{margin}  {pc}   {opcode} {jump}", opcode = inst.opcode);

                println!("{entry:<50}{location}");
            } else if inst.opcode.is_push() {
                let entry = format!(
                    "{margin}  {pc}   {opcode} {push_data}",
                    opcode = inst.opcode,
                    push_data = inst
                        .push_data
                        .as_deref()
                        .map(hex::encode_prefixed)
                        .unwrap_or_default()
                );

                println!("{entry:<50}{location}");
            } else {
                let entry = format!("{margin}  {pc}   {opcode}", opcode = inst.opcode);

                println!("{entry:<50}{location}");
            }
        } else {
            println!("{margin}  {pc}");
        }
    }

    Ok(())
}

#[napi]
fn print_stack_trace(trace: SolidityStackTrace) -> napi::Result<()> {
    let entry_values = trace
        .into_iter()
        .map(|entry| match entry {
            Either24::A(entry) => serde_json::to_value(entry),
            Either24::B(entry) => serde_json::to_value(entry),
            Either24::C(entry) => serde_json::to_value(entry),
            Either24::D(entry) => serde_json::to_value(entry),
            Either24::F(entry) => serde_json::to_value(entry),
            Either24::G(entry) => serde_json::to_value(entry),
            Either24::H(entry) => serde_json::to_value(entry),
            Either24::I(entry) => serde_json::to_value(entry),
            Either24::J(entry) => serde_json::to_value(entry),
            Either24::K(entry) => serde_json::to_value(entry),
            Either24::L(entry) => serde_json::to_value(entry),
            Either24::M(entry) => serde_json::to_value(entry),
            Either24::N(entry) => serde_json::to_value(entry),
            Either24::O(entry) => serde_json::to_value(entry),
            Either24::P(entry) => serde_json::to_value(entry),
            Either24::Q(entry) => serde_json::to_value(entry),
            Either24::R(entry) => serde_json::to_value(entry),
            Either24::S(entry) => serde_json::to_value(entry),
            Either24::T(entry) => serde_json::to_value(entry),
            Either24::U(entry) => serde_json::to_value(entry),
            Either24::V(entry) => serde_json::to_value(entry),
            Either24::W(entry) => serde_json::to_value(entry),
            Either24::X(entry) => serde_json::to_value(entry),
            // Decode the error message from the return data
            Either24::E(entry @ RevertErrorStackTraceEntry { .. }) => {
                use serde::de::Error;

                let decoded_error_msg = ReturnData::new(entry.return_data.clone())
                    .decode_error()
                    .map_err(|e| {
                    serde_json::Error::custom(format_args!("Error decoding return data: {e}"))
                })?;

                let mut value = serde_json::to_value(entry)?;
                value["message"] = decoded_error_msg.into();
                Ok(value)
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| napi::Error::from_reason(format!("Error converting to JSON: {e}")))?;

    println!("{}", serde_json::to_string_pretty(&entry_values)?);

    Ok(())
}
