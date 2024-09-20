use std::rc::Rc;

use edr_solidity::artifacts::{CompilerInput, CompilerOutput};
use napi::{bindgen_prelude::ClassInstance, Env};
use napi_derive::napi;

use crate::trace::model::BytecodeWrapper;

#[napi(catch_unwind)]
pub fn create_models_and_decode_bytecodes(
    solc_version: String,
    compiler_input: serde_json::Value,
    compiler_output: serde_json::Value,
    env: Env,
) -> napi::Result<Vec<ClassInstance<BytecodeWrapper>>> {
    let compiler_input: CompilerInput = serde_json::from_value(compiler_input)?;
    let compiler_output: CompilerOutput = serde_json::from_value(compiler_output)?;

    edr_solidity::compiler::create_models_and_decode_bytecodes(
        solc_version,
        &compiler_input,
        &compiler_output,
    )?
    .into_iter()
    .map(|bytecode| BytecodeWrapper::new(Rc::new(bytecode)).into_instance(env))
    .collect()
}
