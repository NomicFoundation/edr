use std::{collections::HashMap, str::FromStr as _, sync::Arc};

use anyhow::Context as _;
use edr_primitives::{hex, keccak256};
use indexmap::IndexMap;
use parking_lot::RwLock;

use crate::{
    artifacts::{CompilerArtifact, CompilerOutput, ContractAbiEntry},
    build_model::{
        Contract, ContractFunction, ContractFunctionType, ContractFunctionVisibility, ContractKind,
        SourceFile, SourceLocation,
    },
};

pub(super) fn process_ast_nodes<ArtifactT: CompilerArtifact>(
    file: &mut SourceFile,
    source_name: &str,
    ast: &serde_json::Value,
    file_id_to_source_file: &HashMap<u32, Arc<RwLock<SourceFile>>>,
    compiler_output: &CompilerOutput<ArtifactT>,
    contract_id_to_linearized_base_contract_ids: &mut HashMap<u32, Vec<u32>>,
    contract_id_to_contract: &mut IndexMap<u32, Arc<RwLock<Contract>>>,
) -> anyhow::Result<()> {
    let mut functions = Vec::new();

    let nodes = ast["nodes"]
        .as_array()
        .with_context(|| "Expected nodes array in AST")?;

    for node in nodes {
        match node["nodeType"]
            .as_str()
            .with_context(|| "Expected nodeType to be a string")?
        {
            "ContractDefinition" => {
                let Some(contract_type) = node["contractKind"]
                    .as_str()
                    .and_then(|k| ContractKind::from_str(k).ok())
                else {
                    continue;
                };

                let contract_abi =
                    compiler_output
                        .contracts
                        .get(source_name)
                        .and_then(|contracts| {
                            contracts
                                .get(
                                    node["name"]
                                        .as_str()
                                        .with_context(|| "Expected contract name to be a string")
                                        .ok()?,
                                )
                                .map(|contract| &contract.abi)
                        });

                let ProcessContractResult {
                    contract_id,
                    contract,
                    functions: contract_functions,
                } = process_contract_ast_node(
                    node,
                    contract_type,
                    file_id_to_source_file,
                    contract_id_to_linearized_base_contract_ids,
                    contract_abi.map(Vec::as_slice),
                )?;

                functions.extend(contract_functions);

                contract_id_to_contract.insert(contract_id, contract);
            }
            // top-level functions
            "FunctionDefinition" => {
                if let Some(function) =
                    process_function_definition_ast_node(node, file_id_to_source_file, None, None)?
                {
                    functions.push(function);
                }
            }
            _ => {}
        }
    }

    file.finalize(functions.into_boxed_slice());

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
    sig.get(..4)
        .expect("signature should have at least 4 bytes")
        .to_vec()
}

fn ast_function_definition_to_selector(
    function_definition: &serde_json::Value,
) -> anyhow::Result<Vec<u8>> {
    if let Some(function_selector) = function_definition["functionSelector"].as_str() {
        return hex::decode(function_selector)
            .with_context(|| format!("Failed to decode hex: {function_selector:?}"));
    }

    let mut param_types = Vec::new();

    for param in function_definition
        .get("parameters")
        .expect("function_definition should have parameters")
        .get("parameters")
        .expect("parameters should have parameters")
        .as_array()
        .with_context(|| "Expected function parameters to be an array")?
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
                typename
                    .get("typeDescriptions")
                    .expect("typename should have typeDescriptions")
                    .get("typeString")
                    .expect("typeDescriptions should have typeString")
                    .as_str()
                    .with_context(|| "Expected typeString to be a string")?
                    .to_string(),
            );
            continue;
        }

        param_types.push(to_canonical_abi_type(
            typename["name"]
                .as_str()
                .with_context(|| "Expected typename name to be a string")?,
        ));
    }

    Ok(abi_method_id(
        function_definition["name"]
            .as_str()
            .with_context(|| "Expected function name to be a string")?,
        param_types,
    ))
}

fn ast_src_to_source_location(
    src: &str,
    file_id_to_source_file: &HashMap<u32, Arc<RwLock<SourceFile>>>,
) -> anyhow::Result<Option<SourceLocation>> {
    let parts: Vec<&str> = src.split(':').collect();
    if parts.len() != 3 {
        return Ok(None);
    }

    let offset = parts
        .first()
        .expect("parts should have three elements")
        .parse::<u32>()
        .with_context(|| format!("Failed to parse offset: {src:?}"))?;
    let length = parts
        .get(1)
        .expect("parts should have three elements")
        .parse::<u32>()
        .with_context(|| format!("Failed to parse length: {src:?}"))?;
    let file_id = parts
        .get(2)
        .expect("parts should have three elements")
        .parse::<u32>()
        .with_context(|| format!("Failed to parse file ID: {src:?}"))?;

    if let Some(source_file) = file_id_to_source_file.get(&file_id) {
        Ok(Some(SourceLocation::new(source_file, offset, length)))
    } else {
        Err(anyhow::anyhow!("Failed to find file by ID: {file_id}"))
    }
}

fn canonical_abi_type_for_elementary_or_user_defined_types(
    key_type: &serde_json::Value,
) -> Option<String> {
    if is_elementary_type(key_type) {
        return key_type["name"].as_str().map(to_canonical_abi_type);
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
        Some("constructor") => ContractFunctionType::Constructor,
        Some("fallback") => ContractFunctionType::Fallback,
        Some("receive") => ContractFunctionType::Receive,
        Some("freeFunction") => ContractFunctionType::FreeFunction,
        _ => ContractFunctionType::Function,
    }
}

fn get_public_variable_selector_from_declaration_ast_node(
    variable_declaration: &serde_json::Value,
) -> anyhow::Result<Vec<u8>> {
    if let Some(function_selector) = variable_declaration["functionSelector"].as_str() {
        return hex::decode(function_selector)
            .with_context(|| format!("Failed to decode hex: {function_selector:?}"));
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
                    .with_context(|| "Original code asserted that".to_string())?;

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
        variable_declaration["name"]
            .as_str()
            .with_context(|| "Expected variable name to be a string")?,
        param_types,
    );

    Ok(method_id)
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
            .is_some_and(|s| s.starts_with("contract "))
}

fn is_elementary_type(param: &serde_json::Value) -> bool {
    param["nodeType"] == "ElementaryTypeName" || param["type"] == "ElementaryTypeName"
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
            .is_some_and(|s| s.starts_with("enum "))
}

struct ProcessContractResult {
    pub contract_id: u32,
    pub contract: Arc<RwLock<Contract>>,
    pub functions: Vec<Arc<ContractFunction>>,
}

fn process_contract_ast_node(
    contract_node: &serde_json::Value,
    contract_type: ContractKind,
    file_id_to_source_file: &HashMap<u32, Arc<RwLock<SourceFile>>>,
    contract_id_to_linearized_base_contract_ids: &mut HashMap<u32, Vec<u32>>,
    contract_abi: Option<&[ContractAbiEntry]>,
) -> anyhow::Result<ProcessContractResult> {
    let mut functions = Vec::new();
    let contract_location = ast_src_to_source_location(
        contract_node["src"]
            .as_str()
            .with_context(|| "Expected contract src to be a string")?,
        file_id_to_source_file,
    )?
    .with_context(|| "The original JS code always asserts that".to_string())?;

    let mut contract = Contract::new(
        contract_node["name"]
            .as_str()
            .with_context(|| "Expected contract name to be a string")?
            .to_string(),
        contract_type,
        contract_location,
    );

    let contract_id = contract_node["id"]
        .as_u64()
        .with_context(|| "Expected contract id to be a number")? as u32;

    contract_id_to_linearized_base_contract_ids.insert(
        contract_id,
        contract_node["linearizedBaseContracts"]
            .as_array()
            .with_context(|| "Expected linearizedBaseContracts to be an array")?
            .iter()
            .map(|x| {
                x.as_u64()
                    .with_context(|| "Expected linearizedBaseContract id to be a number")
                    .map(|id| id as u32)
            })
            .collect::<Result<Vec<_>, _>>()?,
    );

    for node in contract_node["nodes"]
        .as_array()
        .with_context(|| "Expected contract nodes to be an array")?
    {
        match node["nodeType"]
            .as_str()
            .with_context(|| "Expected nodeType to be a string")?
        {
            "FunctionDefinition" => {
                let function_abis = contract_abi.map(|contract_abi| {
                    contract_abi
                        .iter()
                        .filter(|abi_entry| abi_entry.name.as_deref() == node["name"].as_str())
                        .collect::<Vec<_>>()
                });

                if let Some(function) = process_function_definition_ast_node(
                    node,
                    file_id_to_source_file,
                    Some(&contract),
                    function_abis,
                )? {
                    contract.add_local_function(function.clone())?;
                    functions.push(function);
                }
            }
            "ModifierDefinition" => {
                let function =
                    process_modifier_definition_ast_node(node, file_id_to_source_file, &contract)?;

                contract.add_local_function(function.clone())?;
                functions.push(function);
            }
            "VariableDeclaration" => {
                let getter_abi = contract_abi.and_then(|contract_abi| {
                    contract_abi
                        .iter()
                        .find(|abi_entry| abi_entry.name.as_deref() == node["name"].as_str())
                });

                if let Some(function) = process_variable_declaration_ast_node(
                    node,
                    file_id_to_source_file,
                    &contract,
                    getter_abi,
                )? {
                    contract.add_local_function(function.clone())?;
                    functions.push(function);
                }
            }
            _ => {}
        }
    }

    Ok(ProcessContractResult {
        contract_id,
        contract: Arc::new(RwLock::new(contract)),
        functions,
    })
}

fn process_function_definition_ast_node(
    node: &serde_json::Value,
    file_id_to_source_file: &HashMap<u32, Arc<RwLock<SourceFile>>>,
    contract: Option<&Contract>,
    function_abis: Option<Vec<&ContractAbiEntry>>,
) -> anyhow::Result<Option<Arc<ContractFunction>>> {
    if node.get("implemented").and_then(serde_json::Value::as_bool) == Some(false) {
        return Ok(None);
    }

    let function_type = function_definition_kind_to_function_type(node["kind"].as_str());

    let function_location = ast_src_to_source_location(
        node["src"]
            .as_str()
            .with_context(|| "Expected function src to be a string")?,
        file_id_to_source_file,
    )?
    .with_context(|| "The original JS code always asserts that".to_string())?;

    let visibility = {
        let visibility = node["visibility"]
            .as_str()
            .with_context(|| "Expected function visibility to be a string")?;

        ContractFunctionVisibility::from_str(visibility).unwrap_or_default()
    };

    let selector = if function_type == ContractFunctionType::Function
        && (visibility == ContractFunctionVisibility::External
            || visibility == ContractFunctionVisibility::Public)
    {
        Some(ast_function_definition_to_selector(node)?)
    } else {
        None
    };

    // function can be overloaded, match the abi by the selector
    let matching_function_abi = if let Some(function_abis) = function_abis.as_ref() {
        let mut result = None;
        for function_abi in function_abis.iter() {
            let name = match function_abi.name {
                Some(ref name) => name,
                None => continue,
            };

            let input_types = function_abi
                .inputs
                .as_ref()
                .map(|inputs| {
                    inputs
                        .iter()
                        .map(|input| {
                            input["type"]
                                .as_str()
                                .with_context(|| "Expected input type to be a string")
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();

            let function_abi_selector = abi_method_id(name, input_types);

            let matches = match (selector.as_ref(), function_abi_selector) {
                (Some(selector), function_abi_selector) if !function_abi_selector.is_empty() => {
                    *selector == function_abi_selector
                }
                _ => false,
            };

            if matches {
                result = Some(function_abi);
                break;
            }
        }
        result
    } else {
        None
    };

    let param_types = matching_function_abi
        .as_ref()
        .and_then(|abi| abi.inputs.as_ref())
        .cloned();

    let contract_func = ContractFunction {
        name: node["name"]
            .as_str()
            .with_context(|| "Expected function name to be a string")?
            .to_string(),
        r#type: function_type,
        location: function_location,
        contract_name: contract.map(|c| c.name.clone()),
        visibility: Some(visibility),
        is_payable: Some(
            node["stateMutability"]
                .as_str()
                .with_context(|| "Expected stateMutability to be a string")?
                == "payable",
        ),
        selector: RwLock::new(selector),
        param_types,
    };

    let contract_func = Arc::new(contract_func);
    Ok(Some(contract_func))
}

fn process_modifier_definition_ast_node(
    node: &serde_json::Value,
    file_id_to_source_file: &HashMap<u32, Arc<RwLock<SourceFile>>>,
    contract: &Contract,
) -> anyhow::Result<Arc<ContractFunction>> {
    let function_location = ast_src_to_source_location(
        node["src"]
            .as_str()
            .with_context(|| "Expected modifier src to be a string")?,
        file_id_to_source_file,
    )?
    .with_context(|| "The original JS code always asserts that".to_string())?;

    let contract_func = ContractFunction {
        name: node["name"]
            .as_str()
            .with_context(|| "Expected modifier name to be a string")?
            .to_string(),
        r#type: ContractFunctionType::Modifier,
        location: function_location,
        contract_name: Some(contract.name.clone()),
        visibility: None,
        is_payable: None,
        selector: RwLock::new(None),
        param_types: None,
    };

    Ok(Arc::new(contract_func))
}

fn process_variable_declaration_ast_node(
    node: &serde_json::Value,
    file_id_to_source_file: &HashMap<u32, Arc<RwLock<SourceFile>>>,
    contract: &Contract,
    getter_abi: Option<&ContractAbiEntry>,
) -> anyhow::Result<Option<Arc<ContractFunction>>> {
    let visibility = {
        let visibility = node["visibility"]
            .as_str()
            .with_context(|| "Expected variable visibility to be a string")?;

        ContractFunctionVisibility::from_str(visibility).unwrap_or_default()
    };

    // Variables can't be external
    if visibility != ContractFunctionVisibility::Public {
        return Ok(None);
    }

    let function_location = ast_src_to_source_location(
        node["src"]
            .as_str()
            .with_context(|| "Expected variable src to be a string")?,
        file_id_to_source_file,
    )?
    .with_context(|| "The original JS code always asserts that".to_string())?;

    let param_types = getter_abi
        .as_ref()
        .and_then(|abi| abi.inputs.as_ref())
        .cloned();

    let contract_func = ContractFunction {
        name: node["name"]
            .as_str()
            .with_context(|| "Expected variable name to be a string")?
            .to_string(),
        r#type: ContractFunctionType::Getter,
        location: function_location,
        contract_name: Some(contract.name.clone()),
        visibility: Some(visibility),
        is_payable: Some(false), // Getters aren't payable
        selector: RwLock::new(Some(
            get_public_variable_selector_from_declaration_ast_node(node)?,
        )),
        param_types,
    };

    Ok(Some(Arc::new(contract_func)))
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
