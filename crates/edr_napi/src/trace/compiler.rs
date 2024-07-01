use edr_evm::hex;
use edr_solidity::artifacts::CompilerOutput;
use napi::{
    bindgen_prelude::{ClassInstance, Object, Undefined},
    Either, Env, JsFunction,
};
use napi_derive::napi;

use super::model::{Bytecode, SourceFile, SourceLocation};

#[napi]
pub fn ast_src_to_source_location(
    src: String,
    #[napi(ts_arg_type = "Map<number, SourceFile>")] file_id_to_source_file: Object,
    env: Env,
) -> napi::Result<Either<ClassInstance<SourceLocation>, Undefined>> {
    let parts: Vec<&str> = src.split(':').collect();
    if parts.len() != 3 {
        return Ok(Either::B(()));
    }

    let offset = parts[0]
        .parse::<u32>()
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse offset: {:?}", e)))?;
    let length = parts[1]
        .parse::<u32>()
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse length: {:?}", e)))?;
    let file_id = parts[2]
        .parse::<u32>()
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse file ID: {:?}", e)))?;

    let file = file_id_to_source_file
        .get_named_property::<JsFunction>("get")?
        .apply1::<u32, Object, ClassInstance<SourceFile>>(file_id_to_source_file, file_id)?;

    SourceLocation::new(file, offset, length, env)
        .and_then(|a| a.into_instance(env))
        .map(Either::A)
}

#[napi]
pub fn correct_selectors(
    bytecodes: Vec<ClassInstance<Bytecode>>,
    compiler_output: serde_json::Value,
    env: Env,
) -> napi::Result<()> {
    let compiler_output: CompilerOutput = serde_json::from_value(compiler_output)?;

    for bytecode in bytecodes.iter().filter(|b| !b.is_deployment) {
        let mut contract = bytecode.contract.as_instance(env)?;
        // Fetch the method identifiers for the contract from the compiler output
        let method_identifiers = match compiler_output
            .contracts
            .get(
                &contract
                    .location
                    .as_instance(env)?
                    .file
                    .as_instance(env)?
                    .source_name,
            )
            .and_then(|file| file.get(&contract.name))
            .map(|contract| &contract.evm.method_identifiers)
        {
            Some(ids) => ids,
            None => continue,
        };

        for (signature, hex_selector) in method_identifiers {
            let function_name = signature.splitn(2, '(').next().unwrap_or("");
            let selector = hex::decode(&hex_selector)
                .map_err(|e| napi::Error::from_reason(format!("Failed to decode hex: {:?}", e)))?;

            let contract_function =
                contract.get_function_from_selector(selector.clone().into(), env)?;

            if let Either::A(_) = contract_function {
                continue;
            }

            // TODO: This code path is not covered by any of the existing tests.
            // Let's create a stack trace that exercises that code path or
            // let's remove it if/when we adapt our model to also properly
            // support ABI v2.
            let fixed_selector = contract.correct_selector(
                function_name.to_string(),
                selector.clone().into(),
                env,
            )?;

            if !fixed_selector {
                return Err(napi::Error::from_reason(format!(
                    "Failed to compute the selector for one or more implementations of {}#{}. Hardhat Network can automatically fix this problem if you don't use function overloading.",
                    contract.name, function_name
                )));
            }
        }
    }
    Ok(())
}
