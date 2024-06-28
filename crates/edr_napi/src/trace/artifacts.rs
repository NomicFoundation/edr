use napi_derive::napi;

use edr_solidity::artifacts::{CompilerInput, CompilerOutput};

#[napi]
fn deserialize_compiler_input(value: serde_json::Value) -> napi::Result<()> {
    serde_json::from_value::<CompilerInput>(value.clone())
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))
        .map(drop)
}
#[napi]
fn deserialize_compiler_output(value: serde_json::Value) -> napi::Result<()> {
    serde_json::from_value::<CompilerOutput>(value.clone())
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))
        .map(drop)
}
