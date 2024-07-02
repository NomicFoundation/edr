use edr_evm::alloy_primitives::keccak256;
use edr_evm::hex;
use edr_solidity::artifacts::CompilerOutput;
use napi::{
    bindgen_prelude::{ClassInstance, Object, Uint8Array, Undefined},
    Either, Env, JsFunction,
};
use napi_derive::napi;

use super::model::{
    Bytecode, Contract, ContractFunction, ContractFunctionType, ContractFunctionVisibility,
    SourceFile, SourceLocation,
};
use crate::utils::{ClassInstanceRef, ExplicitEitherIntoOption as _};

#[napi(object)]
pub struct ContractAbiEntry {
    pub name: Option<String>,
    pub inputs: Option<Vec<ContractAbiEntryInput>>,
}

#[napi(object)]
pub struct ContractAbiEntryInput {
    #[napi(js_name = "type")]
    pub r#type: String,
}

#[napi]
pub fn process_modifier_definition_ast_node(
    node: serde_json::Value,
    #[napi(ts_arg_type = "Map<number, SourceFile>")] file_id_to_source_file: Object,
    contract: ClassInstance<Contract>,
    mut file: ClassInstance<SourceFile>,
    env: Env,
) -> napi::Result<()> {
    let function_location = ast_src_to_source_location(
        node["src"].as_str().unwrap().to_string(),
        file_id_to_source_file,
        env,
    )?
    .into_option()
    .expect("The original JS code always asserts that");

    let contract = ClassInstanceRef::from_obj(contract, env)?;

    let contract_func = ContractFunction::new(
        node["name"].as_str().unwrap().to_string(),
        ContractFunctionType::MODIFIER,
        function_location,
        Some(contract.as_instance(env)?),
        None,
        None,
        None,
        None,
        env,
    )?
    .into_instance(env)?;

    let contract_func = ClassInstanceRef::from_obj(contract_func, env)?;
    contract.as_instance(env)?.add_local_function(
        contract_func.as_instance(env)?,
        contract.as_inner(env)?,
        env,
    )?;
    file.add_function(contract_func.as_instance(env)?, env)?;

    Ok(())
}

#[napi]
pub fn process_variable_declaration_ast_node(
    node: serde_json::Value,
    #[napi(ts_arg_type = "Map<number, SourceFile>")] file_id_to_source_file: Object,
    contract: ClassInstance<Contract>,
    mut file: ClassInstance<SourceFile>,
    getter_abi: Option<ContractAbiEntry>,
    env: Env,
) -> napi::Result<()> {
    let visibility = ast_visibility_to_visibility(node["visibility"].as_str().unwrap().to_string());

    // Variables can't be external
    if visibility != ContractFunctionVisibility::PUBLIC {
        return Ok(());
    }

    let function_location = ast_src_to_source_location(
        node["src"].as_str().unwrap().to_string(),
        file_id_to_source_file,
        env,
    )?
    .into_option()
    .expect("The original JS code always asserts that");

    let param_types = getter_abi
        .as_ref()
        .and_then(|getter_abi| getter_abi.inputs.as_ref())
        .map(|inputs| {
            inputs
                .iter()
                .map(|input| input.r#type.clone())
                .collect::<Vec<_>>()
        });

    let contract = ClassInstanceRef::from_obj(contract, env)?;

    let contract_func = ContractFunction::new(
        node["name"].as_str().unwrap().to_string(),
        ContractFunctionType::GETTER,
        function_location,
        Some(contract.as_instance(env)?),
        Some(visibility),
        Some(false), // Getters aren't payable
        Some(Uint8Array::from(
            get_public_variable_selector_from_declaration_ast_node(node)?,
        )),
        Some(param_types.into_iter().map(|v| v.into()).collect()),
        env,
    )?
    .into_instance(env)?;

    let contract_func = ClassInstanceRef::from_obj(contract_func, env)?;
    contract.as_instance(env)?.add_local_function(
        contract_func.as_instance(env)?,
        contract.as_inner(env)?,
        env,
    )?;
    file.add_function(contract_func.as_instance(env)?, env)?;

    Ok(())
}

fn get_public_variable_selector_from_declaration_ast_node(
    variable_declaration: serde_json::Value,
) -> napi::Result<Vec<u8>> {
    if let Some(function_selector) = variable_declaration["functionSelector"].as_str() {
        return Ok(hex::decode(function_selector)
            .map_err(|e| napi::Error::from_reason(format!("Failed to decode hex: {:?}", e)))?);
    }

    // TODO: It seems we don't have tests that exercise missing functionSelector
    // in the variable declaration
    let mut param_types = Vec::new();

    // VariableDeclaration nodes for function parameters or state variables will always
    // have their typeName fields defined.
    let mut next_type = &variable_declaration["typeName"];
    loop {
        if next_type["nodeType"] == "Mapping" {
            let canonical_type =
                canonical_abi_type_for_elementary_or_user_defined_types(&next_type["keyType"])
                    .expect("Original code asserted that");

            param_types.push(canonical_type);

            next_type = &next_type["valueType"];
        } else {
            if next_type["nodeType"] == "ArrayTypeName" {
                param_types.push("uint256".to_string());
            }

            break;
        }
    }

    let method_id = abi_method_id(
        variable_declaration["name"].as_str().unwrap(),
        param_types.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
    );

    Ok(method_id)
}

fn canonical_abi_type_for_elementary_or_user_defined_types(
    key_type: &serde_json::Value,
) -> Option<String> {
    if is_elementary_type(&key_type) {
        return Some(to_canonical_abi_type(
            key_type["name"].as_str().unwrap().to_string(),
        ));
    }

    if is_enum_type(key_type.clone()) {
        return Some("uint256".to_string());
    }

    if is_contract_type(key_type.clone()) {
        return Some("address".to_string());
    }

    None
}

#[napi]
fn ast_visibility_to_visibility(visibility: String) -> ContractFunctionVisibility {
    match &*visibility {
        "private" => ContractFunctionVisibility::PRIVATE,
        "internal" => ContractFunctionVisibility::INTERNAL,
        "public" => ContractFunctionVisibility::PUBLIC,
        _ => ContractFunctionVisibility::EXTERNAL,
    }
}

#[napi]
pub fn is_contract_type(param: serde_json::Value) -> bool {
    (param
        .pointer("/typeName/nodeType")
        .and_then(serde_json::Value::as_str)
        == Some("UserDefinedTypeName")
        || param.get("nodeType").and_then(serde_json::Value::as_str) == Some("UserDefinedTypeName"))
        && param
            .pointer("/typeDescriptions/typeString")
            .and_then(serde_json::Value::as_str)
            .map_or(false, |s| s.starts_with("contract "))
}

#[napi]
pub fn is_enum_type(param: serde_json::Value) -> bool {
    (param
        .pointer("/typeName/nodeType")
        .and_then(serde_json::Value::as_str)
        == Some("UserDefinedTypeName")
        || param.get("nodeType").and_then(serde_json::Value::as_str) == Some("UserDefinedTypeName"))
        && param
            .pointer("/typeDescriptions/typeString")
            .and_then(serde_json::Value::as_str)
            .map_or(false, |s| s.starts_with("enum "))
}

pub fn is_elementary_type(param: &serde_json::Value) -> bool {
    param["nodeType"] == "ElementaryTypeName" || param["type"] == "ElementaryTypeName"
}

#[napi]
pub fn to_canonical_abi_type(type_: String) -> String {
    if type_.starts_with("int[") {
        return format!("int256{}", &type_[3..]);
    }
    if type_ == "int" {
        return "int256".to_string();
    }
    if type_.starts_with("uint[") {
        return format!("uint256{}", &type_[4..]);
    }
    if type_ == "uint" {
        return "uint256".to_string();
    }
    if type_.starts_with("fixed[") {
        return format!("fixed128x128{}", &type_[5..]);
    }
    if type_ == "fixed" {
        return "fixed128x128".to_string();
    }
    if type_.starts_with("ufixed[") {
        return format!("ufixed128x128{}", &type_[6..]);
    }
    if type_ == "ufixed" {
        return "ufixed128x128".to_string();
    }

    type_
}

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

fn abi_method_id(name: &str, param_types: Vec<impl Into<String>>) -> Vec<u8> {
    let sig = format!(
        "{name}({})",
        // wasteful, but it's fine for now
        param_types
            .into_iter()
            .map(|x| to_canonical_abi_type(x.into()))
            .collect::<Vec<_>>()
            .join(",")
    );
    let sig = sig.as_bytes();
    let sig = keccak256(sig);
    sig[..4].to_vec()
}
