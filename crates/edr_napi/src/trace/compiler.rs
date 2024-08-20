//! Processes the AST and compiler input and creates the source model.
//! Ported from `hardhat-network/stack-traces/compiler-to-model.ts`.

use std::cell::RefCell;
use std::str::FromStr;
use std::{collections::HashMap, rc::Rc};

use edr_evm::{alloy_primitives::keccak256, hex};
use edr_solidity::{
    artifacts::{CompilerInput, CompilerOutput, CompilerOutputBytecode, ContractAbiEntry},
    library_utils::get_library_address_positions,
};
use indexmap::IndexMap;
use napi::bindgen_prelude::ClassInstance;
use napi::Env;
use napi_derive::napi;

use super::model::BytecodeWrapper;
use super::{
    library_utils::normalize_compiler_output_bytecode,
    model::{
        Bytecode, Contract, ContractFunction, ContractFunctionType, ContractFunctionVisibility,
        ContractKind, CustomError, SourceFile, SourceLocation,
    },
    source_map::decode_instructions,
};

#[napi]
pub fn create_models_and_decode_bytecodes(
    solc_version: String,
    compiler_input: serde_json::Value,
    compiler_output: serde_json::Value,
    env: Env,
) -> napi::Result<Vec<ClassInstance<BytecodeWrapper>>> {
    let compiler_input: CompilerInput = serde_json::from_value(compiler_input)?;
    let compiler_output: CompilerOutput = serde_json::from_value(compiler_output)?;

    create_models_and_decode_bytecodes_inner(solc_version, &compiler_input, &compiler_output)?
        .into_iter()
        .map(|bytecode| BytecodeWrapper(Rc::new(bytecode)).into_instance(env))
        .collect()
}

pub fn create_models_and_decode_bytecodes_inner(
    solc_version: String,
    compiler_input: &CompilerInput,
    compiler_output: &CompilerOutput,
) -> napi::Result<Vec<Bytecode>> {
    let mut file_id_to_source_file = HashMap::new();
    let mut contract_id_to_contract = IndexMap::new();

    create_sources_model_from_ast(
        compiler_output,
        compiler_input,
        &mut file_id_to_source_file,
        &mut contract_id_to_contract,
    )?;

    let bytecodes = decode_bytecodes(
        solc_version,
        compiler_output,
        &file_id_to_source_file,
        &contract_id_to_contract,
    )?;

    correct_selectors(&bytecodes, compiler_output)?;

    Ok(bytecodes)
}

fn create_sources_model_from_ast(
    compiler_output: &CompilerOutput,
    compiler_input: &CompilerInput,
    file_id_to_source_file: &mut HashMap<u32, Rc<RefCell<SourceFile>>>,
    contract_id_to_contract: &mut IndexMap<u32, Rc<RefCell<Contract>>>,
) -> napi::Result<()> {
    let mut contract_id_to_linearized_base_contract_ids = HashMap::new();

    for (source_name, source) in &compiler_output.sources {
        let file = SourceFile::new(
            source_name.to_string(),
            compiler_input.sources[source_name].content.clone(),
        );
        let file = Rc::new(RefCell::new(file));

        file_id_to_source_file.insert(source.id, file.clone());

        for node in source.ast["nodes"].as_array().unwrap() {
            match node["nodeType"].as_str().unwrap() {
                "ContractDefinition" => {
                    let contract_kind = node["contractKind"].as_str();
                    let contract_type = contract_kind.and_then(|k| ContractKind::from_str(k).ok());

                    let contract_type = match contract_type {
                        Some(contract_type) => contract_type,
                        None => continue,
                    };

                    let contract_abi =
                        compiler_output
                            .contracts
                            .get(source_name)
                            .and_then(|contracts| {
                                contracts
                                    .get(node["name"].as_str().unwrap())
                                    .map(|contract| &contract.abi)
                            });

                    process_contract_ast_node(
                        &file,
                        node,
                        file_id_to_source_file,
                        contract_type,
                        contract_id_to_contract,
                        &mut contract_id_to_linearized_base_contract_ids,
                        contract_abi.map(Vec::as_slice),
                    )?;
                }
                // top-level functions
                "FunctionDefinition" => {
                    process_function_definition_ast_node(
                        node,
                        file_id_to_source_file,
                        None,
                        &file,
                        None,
                    )?;
                }
                _ => {}
            }
        }
    }

    apply_contracts_inheritance(
        contract_id_to_contract,
        &contract_id_to_linearized_base_contract_ids,
    )?;

    Ok(())
}

fn apply_contracts_inheritance(
    contract_id_to_contract: &IndexMap<u32, Rc<RefCell<Contract>>>,
    contract_id_to_linearized_base_contract_ids: &HashMap<u32, Vec<u32>>,
) -> napi::Result<()> {
    for (cid, contract) in contract_id_to_contract {
        let mut contract = contract.borrow_mut();

        let inheritance_ids = &contract_id_to_linearized_base_contract_ids[cid];

        for base_id in inheritance_ids {
            let base_contract = contract_id_to_contract.get(base_id);

            let base_contract = match base_contract {
                Some(base_contract) => base_contract,
                // This list includes interface, which we don't model
                None => continue,
            };

            if cid != base_id {
                let base_contract = &base_contract.borrow();
                contract.add_next_linearized_base_contract(base_contract)?;
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)] // mimick the original code
fn process_contract_ast_node(
    file: &RefCell<SourceFile>,
    contract_node: &serde_json::Value,
    file_id_to_source_file: &HashMap<u32, Rc<RefCell<SourceFile>>>,
    contract_type: ContractKind,
    contract_id_to_contract: &mut IndexMap<u32, Rc<RefCell<Contract>>>,
    contract_id_to_linearized_base_contract_ids: &mut HashMap<u32, Vec<u32>>,
    contract_abi: Option<&[ContractAbiEntry]>,
) -> napi::Result<()> {
    let contract_location = ast_src_to_source_location(
        contract_node["src"].as_str().unwrap(),
        file_id_to_source_file,
    )?
    .expect("The original JS code always asserts that");

    let contract = Contract::new(
        contract_node["name"].as_str().unwrap().to_string(),
        contract_type,
        contract_location,
    )?;
    let contract = Rc::new(RefCell::new(contract));

    let contract_id = contract_node["id"].as_u64().unwrap() as u32;
    contract_id_to_contract.insert(contract_id, contract.clone());

    contract_id_to_linearized_base_contract_ids.insert(
        contract_id,
        contract_node["linearizedBaseContracts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap() as u32)
            .collect(),
    );

    for node in contract_node["nodes"].as_array().unwrap() {
        match node["nodeType"].as_str().unwrap() {
            "FunctionDefinition" => {
                let function_abis = contract_abi.map(|contract_abi| {
                    contract_abi
                        .iter()
                        .filter(|abi_entry| abi_entry.name.as_deref() == node["name"].as_str())
                        .collect::<Vec<_>>()
                });

                process_function_definition_ast_node(
                    node,
                    file_id_to_source_file,
                    Some(&contract),
                    file,
                    function_abis,
                )?;
            }
            "ModifierDefinition" => {
                process_modifier_definition_ast_node(
                    node,
                    file_id_to_source_file,
                    &contract,
                    file,
                )?;
            }
            "VariableDeclaration" => {
                let getter_abi = contract_abi.and_then(|contract_abi| {
                    contract_abi
                        .iter()
                        .find(|abi_entry| abi_entry.name.as_deref() == node["name"].as_str())
                });

                process_variable_declaration_ast_node(
                    node,
                    file_id_to_source_file,
                    &contract,
                    file,
                    getter_abi,
                )?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn process_function_definition_ast_node(
    node: &serde_json::Value,
    file_id_to_source_file: &HashMap<u32, Rc<RefCell<SourceFile>>>,
    contract: Option<&RefCell<Contract>>,
    file: &RefCell<SourceFile>,
    function_abis: Option<Vec<&ContractAbiEntry>>,
) -> napi::Result<()> {
    if node.get("implemented").and_then(serde_json::Value::as_bool) == Some(false) {
        return Ok(());
    }

    let function_type = function_definition_kind_to_function_type(node["kind"].as_str());

    let function_location =
        ast_src_to_source_location(node["src"].as_str().unwrap(), file_id_to_source_file)?
            .expect("The original JS code always asserts that");

    let visibility = ast_visibility_to_visibility(node["visibility"].as_str().unwrap());

    let selector = if function_type == ContractFunctionType::FUNCTION
        && (visibility == ContractFunctionVisibility::External
            || visibility == ContractFunctionVisibility::Public)
    {
        Some(ast_function_definition_to_selector(node)?)
    } else {
        None
    };

    // function can be overloaded, match the abi by the selector
    let matching_function_abi = function_abis.as_ref().and_then(|function_abis| {
        function_abis.iter().find(|function_abi| {
            let name = match function_abi.name {
                Some(ref name) => name,
                None => return false,
            };

            let function_abi_selector = abi_method_id(
                name,
                function_abi
                    .inputs
                    .as_ref()
                    .map(|inputs| {
                        inputs
                            .iter()
                            .map(|input| input["type"].as_str().unwrap())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            );

            match (selector.as_ref(), function_abi_selector) {
                (Some(selector), function_abi_selector) if !function_abi_selector.is_empty() => {
                    selector.as_ref() == function_abi_selector
                }
                _ => false,
            }
        })
    });

    let param_types = matching_function_abi
        .as_ref()
        .and_then(|abi| abi.inputs.as_ref())
        .cloned();

    let contract_func = ContractFunction {
        name: node["name"].as_str().unwrap().to_string(),
        r#type: function_type,
        location: function_location,
        contract_name: contract
            .as_ref()
            .map(|c| c.borrow())
            .map(|c| c.name.clone()),
        visibility: Some(visibility),
        is_payable: Some(node["stateMutability"].as_str().unwrap() == "payable"),
        selector: RefCell::new(selector),
        param_types,
    };
    let contract_func = Rc::new(contract_func);

    if let Some(contract) = contract {
        let mut contract = contract.borrow_mut();
        contract.add_local_function(contract_func.clone())?;
    }

    let mut file = file
        .try_borrow_mut()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    file.add_function(contract_func);

    Ok(())
}

fn process_modifier_definition_ast_node(
    node: &serde_json::Value,
    file_id_to_source_file: &HashMap<u32, Rc<RefCell<SourceFile>>>,
    contract: &RefCell<Contract>,
    file: &RefCell<SourceFile>,
) -> napi::Result<()> {
    let function_location =
        ast_src_to_source_location(node["src"].as_str().unwrap(), file_id_to_source_file)?
            .expect("The original JS code always asserts that");

    let contract_func = ContractFunction {
        name: node["name"].as_str().unwrap().to_string(),
        r#type: ContractFunctionType::MODIFIER,
        location: function_location,
        contract_name: Some(contract.borrow().name.clone()),
        visibility: None,
        is_payable: None,
        selector: RefCell::new(None),
        param_types: None,
    };

    let contract_func = Rc::new(contract_func);

    let mut contract = contract.borrow_mut();
    let mut file = file
        .try_borrow_mut()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    contract.add_local_function(contract_func.clone())?;
    file.add_function(contract_func.clone());

    Ok(())
}

fn process_variable_declaration_ast_node(
    node: &serde_json::Value,
    file_id_to_source_file: &HashMap<u32, Rc<RefCell<SourceFile>>>,
    contract: &RefCell<Contract>,
    file: &RefCell<SourceFile>,
    getter_abi: Option<&ContractAbiEntry>,
) -> napi::Result<()> {
    let visibility = ast_visibility_to_visibility(node["visibility"].as_str().unwrap());

    // Variables can't be external
    if visibility != ContractFunctionVisibility::Public {
        return Ok(());
    }

    let function_location =
        ast_src_to_source_location(node["src"].as_str().unwrap(), file_id_to_source_file)?
            .expect("The original JS code always asserts that");

    let param_types = getter_abi
        .as_ref()
        .and_then(|abi| abi.inputs.as_ref())
        .cloned();

    let contract_func = ContractFunction {
        name: node["name"].as_str().unwrap().to_string(),
        r#type: ContractFunctionType::GETTER,
        location: function_location,
        contract_name: Some(contract.borrow().name.clone()),
        visibility: Some(visibility),
        is_payable: Some(false), // Getters aren't payable
        selector: RefCell::new(Some(
            get_public_variable_selector_from_declaration_ast_node(node)?,
        )),
        param_types,
    };
    let contract_func = Rc::new(contract_func);

    let mut contract = contract.borrow_mut();
    let mut file = file
        .try_borrow_mut()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    contract.add_local_function(contract_func.clone())?;
    file.add_function(contract_func);

    Ok(())
}

fn get_public_variable_selector_from_declaration_ast_node(
    variable_declaration: &serde_json::Value,
) -> napi::Result<Vec<u8>> {
    if let Some(function_selector) = variable_declaration["functionSelector"].as_str() {
        return hex::decode(function_selector)
            .map_err(|e| napi::Error::from_reason(format!("Failed to decode hex: {e:?}")));
    }

    // NOTE: It seems we don't have tests that exercise missing functionSelector
    // in the variable declaration
    let mut param_types = Vec::new();

    // VariableDeclaration nodes for function parameters or state variables will
    // always have their typeName fields defined.
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

    let method_id = abi_method_id(variable_declaration["name"].as_str().unwrap(), param_types);

    Ok(method_id)
}

fn ast_function_definition_to_selector(
    function_definition: &serde_json::Value,
) -> napi::Result<Vec<u8>> {
    if let Some(function_selector) = function_definition["functionSelector"].as_str() {
        return hex::decode(function_selector)
            .map_err(|e| napi::Error::from_reason(format!("Failed to decode hex: {e:?}")));
    }

    let mut param_types = Vec::new();

    for param in function_definition["parameters"]["parameters"]
        .as_array()
        .unwrap()
    {
        if is_contract_type(param) {
            param_types.push("address".to_string());
            continue;
        }

        // TODO: implement ABIv2 structs parsing
        // This might mean we need to parse struct definitions before
        // resolving types and trying to calculate function selectors.
        // if is_struct_type(param) {
        //   param_types.push(something);
        //   continue;
        // }

        if is_enum_type(param) {
            // TODO: If the enum has >= 256 elements this will fail. It should be a uint16.
            // This is  complicated, as enums can be inherited. Fortunately, if
            // multiple parent contracts  define the same enum, solc fails to
            // compile.
            param_types.push("uint8".to_string());
            continue;
        }

        let typename = &param["typeName"];
        let node_type = param
            .pointer("/typeName/nodeType")
            .and_then(serde_json::Value::as_str);
        if matches!(
            node_type,
            Some("ArrayTypeName" | "FunctionTypeName" | "Mapping")
        ) {
            param_types.push(
                typename["typeDescriptions"]["typeString"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            );
            continue;
        }

        param_types.push(to_canonical_abi_type(typename["name"].as_str().unwrap()));
    }

    Ok(abi_method_id(
        function_definition["name"].as_str().unwrap(),
        param_types,
    ))
}

fn canonical_abi_type_for_elementary_or_user_defined_types(
    key_type: &serde_json::Value,
) -> Option<String> {
    if is_elementary_type(key_type) {
        return Some(to_canonical_abi_type(key_type["name"].as_str().unwrap()));
    }

    if is_enum_type(key_type) {
        return Some("uint256".to_string());
    }

    if is_contract_type(key_type) {
        return Some("address".to_string());
    }

    None
}

fn function_definition_kind_to_function_type(kind: Option<&str>) -> ContractFunctionType {
    match kind {
        Some("constructor") => ContractFunctionType::CONSTRUCTOR,
        Some("fallback") => ContractFunctionType::FALLBACK,
        Some("receive") => ContractFunctionType::RECEIVE,
        Some("freeFunction") => ContractFunctionType::FREE_FUNCTION,
        _ => ContractFunctionType::FUNCTION,
    }
}

fn ast_visibility_to_visibility(visibility: &str) -> ContractFunctionVisibility {
    match visibility {
        "private" => ContractFunctionVisibility::Private,
        "internal" => ContractFunctionVisibility::Internal,
        "public" => ContractFunctionVisibility::Public,
        _ => ContractFunctionVisibility::External,
    }
}

fn is_contract_type(param: &serde_json::Value) -> bool {
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

fn is_enum_type(param: &serde_json::Value) -> bool {
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

fn is_elementary_type(param: &serde_json::Value) -> bool {
    param["nodeType"] == "ElementaryTypeName" || param["type"] == "ElementaryTypeName"
}

fn to_canonical_abi_type(type_: &str) -> String {
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

    type_.to_owned()
}

fn ast_src_to_source_location(
    src: &str,
    file_id_to_source_file: &HashMap<u32, Rc<RefCell<SourceFile>>>,
) -> napi::Result<Option<Rc<SourceLocation>>> {
    let parts: Vec<&str> = src.split(':').collect();
    if parts.len() != 3 {
        return Ok(None);
    }

    let offset = parts[0]
        .parse::<u32>()
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse offset: {e:?}")))?;
    let length = parts[1]
        .parse::<u32>()
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse length: {e:?}")))?;
    let file_id = parts[2]
        .parse::<u32>()
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse file ID: {e:?}")))?;

    let file = file_id_to_source_file
        .get(&file_id)
        .ok_or_else(|| napi::Error::from_reason("Failed to find file by ID"))?;

    Ok(Some(Rc::new(SourceLocation::new(
        file.clone(),
        offset,
        length,
    ))))
}

fn correct_selectors(bytecodes: &[Bytecode], compiler_output: &CompilerOutput) -> napi::Result<()> {
    for bytecode in bytecodes.iter().filter(|b| !b.is_deployment) {
        let mut contract = bytecode.contract.borrow_mut();
        // Fetch the method identifiers for the contract from the compiler output
        let method_identifiers = match compiler_output
            .contracts
            .get(
                &contract
                    .location
                    .file
                    .try_borrow()
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?
                    .source_name,
            )
            .and_then(|file| file.get(&contract.name))
            .map(|contract| &contract.evm.method_identifiers)
        {
            Some(ids) => ids,
            None => continue,
        };

        for (signature, hex_selector) in method_identifiers {
            let function_name = signature.split('(').next().unwrap_or("");
            let selector = hex::decode(hex_selector)
                .map_err(|e| napi::Error::from_reason(format!("Failed to decode hex: {e:?}")))?;

            let contract_function = contract.get_function_from_selector(&selector);

            if contract_function.is_some() {
                continue;
            }

            // NOTE: This code path is not covered by any of the existing tests.
            // Let's create a stack trace that exercises that code path or
            // let's remove it if/when we adapt our model to also properly
            // support ABI v2.
            let fixed_selector =
                contract.correct_selector(function_name.to_string(), selector.clone())?;

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

fn abi_method_id(name: &str, param_types: Vec<impl AsRef<str>>) -> Vec<u8> {
    let sig = format!(
        "{name}({})",
        // wasteful, but it's fine for now
        param_types
            .into_iter()
            .map(|x| to_canonical_abi_type(x.as_ref()))
            .collect::<Vec<_>>()
            .join(",")
    );
    let sig = sig.as_bytes();
    let sig = keccak256(sig);
    sig[..4].to_vec()
}

fn decode_evm_bytecode(
    contract: Rc<RefCell<Contract>>,
    solc_version: String,
    is_deployment: bool,
    compiler_bytecode: &CompilerOutputBytecode,
    file_id_to_source_file: &HashMap<u32, Rc<RefCell<SourceFile>>>,
) -> napi::Result<Bytecode> {
    let library_address_positions = get_library_address_positions(compiler_bytecode);

    let immutable_references = compiler_bytecode
        .immutable_references
        .as_ref()
        .map(|refs| {
            refs.values()
                .flatten()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let normalized_code = normalize_compiler_output_bytecode(
        compiler_bytecode.object.clone(),
        &library_address_positions,
    )
    .map_err(|e| napi::Error::from_reason(format!("Failed to decode hex: {e:?}")))?;

    let instructions = decode_instructions(
        &normalized_code,
        &compiler_bytecode.source_map,
        file_id_to_source_file,
        is_deployment,
    );

    Ok(Bytecode::new(
        contract,
        is_deployment,
        normalized_code,
        instructions,
        library_address_positions,
        immutable_references,
        solc_version,
    ))
}

fn decode_bytecodes(
    solc_version: String,
    compiler_output: &CompilerOutput,
    file_id_to_source_file: &HashMap<u32, Rc<RefCell<SourceFile>>>,
    contract_id_to_contract: &IndexMap<u32, Rc<RefCell<Contract>>>,
) -> napi::Result<Vec<Bytecode>> {
    let mut bytecodes = Vec::new();

    for contract in contract_id_to_contract.values() {
        let contract_rc = contract.clone();

        let mut contract = contract.borrow_mut();

        let contract_file = &contract
            .location
            .file
            .try_borrow()
            .map_err(|e| napi::Error::from_reason(e.to_string()))?
            .source_name
            .clone();
        let contract_evm_output = &compiler_output.contracts[contract_file][&contract.name].evm;
        let contract_abi_output = &compiler_output.contracts[contract_file][&contract.name].abi;

        for item in contract_abi_output {
            if item.r#type.as_deref() == Some("error") {
                if let Ok(custom_error) = CustomError::from_abi(item.clone()) {
                    contract.add_custom_error(custom_error);
                }
            }
        }

        // This is an abstract contract
        if contract_evm_output.bytecode.object.is_empty() {
            continue;
        }

        let deployment_bytecode = decode_evm_bytecode(
            contract_rc.clone(),
            solc_version.clone(),
            true,
            &contract_evm_output.bytecode,
            file_id_to_source_file,
        )?;

        let runtime_bytecode = decode_evm_bytecode(
            contract_rc.clone(),
            solc_version.clone(),
            false,
            &contract_evm_output.deployed_bytecode,
            file_id_to_source_file,
        )?;

        bytecodes.push(deployment_bytecode);
        bytecodes.push(runtime_bytecode);
    }

    Ok(bytecodes)
}
