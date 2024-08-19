//! Ported from `hardhat-network/stack-traces/model.ts`.

use std::{
    cell::{OnceCell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use alloy_dyn_abi::ErrorExt;
use edr_evm::hex;
use edr_solidity::artifacts::ContractAbiEntry;
use napi_derive::napi;
use serde::Serialize;
use serde_json::Value;

use super::opcodes::Opcode;

pub struct SourceFile {
    // Referenced because it can be later updated by outside code
    functions: Vec<Rc<ContractFunction>>,

    pub source_name: String,
    pub content: String,
}

impl SourceFile {
    pub fn new(source_name: String, content: String) -> SourceFile {
        SourceFile {
            functions: Vec::new(),

            content,
            source_name,
        }
    }

    pub fn add_function(&mut self, contract_function: Rc<ContractFunction>) {
        self.functions.push(contract_function);
    }

    pub fn get_containing_function(
        &self,
        location: &SourceLocation,
    ) -> napi::Result<Option<&Rc<ContractFunction>>> {
        for func in &self.functions {
            if func.location.contains(location) {
                return Ok(Some(func));
            }
        }

        Ok(None)
    }
}

#[derive(Clone)]
pub struct SourceLocation {
    line: OnceCell<u32>,
    pub(crate) file: Rc<RefCell<SourceFile>>,
    pub offset: u32,
    pub length: u32,
}

impl SourceLocation {
    pub fn new(file: Rc<RefCell<SourceFile>>, offset: u32, length: u32) -> SourceLocation {
        SourceLocation {
            line: OnceCell::new(),
            file,
            offset,
            length,
        }
    }

    pub fn get_starting_line_number(&self) -> napi::Result<u32> {
        if let Some(line) = self.line.get() {
            return Ok(*line);
        }

        let contents = &self
            .file
            .try_borrow()
            .map_err(|e| napi::Error::from_reason(e.to_string()))?
            .content;

        Ok(*self.line.get_or_init(move || {
            let mut line = 1;

            for c in contents.chars().take(self.offset as usize) {
                if c == '\n' {
                    line += 1;
                }
            }

            line
        }))
    }

    pub fn get_containing_function(&self) -> napi::Result<Option<Rc<ContractFunction>>> {
        Ok(self
            .file
            .try_borrow()
            .map_err(|e| napi::Error::from_reason(e.to_string()))?
            .get_containing_function(self)?
            .cloned())
    }

    pub fn contains(&self, other: &SourceLocation) -> bool {
        if !Rc::ptr_eq(&self.file, &other.file) {
            return false;
        }

        if other.offset < self.offset {
            return false;
        }

        other.offset + other.length <= self.offset + self.length
    }

    pub fn equals(&self, other: &SourceLocation) -> bool {
        Rc::ptr_eq(&self.file, &other.file)
            && self.offset == other.offset
            && self.length == other.length
    }
}

#[derive(PartialEq, Eq, Serialize)]
#[allow(non_camel_case_types)] // intentionally mimicks the original case in TS
#[allow(clippy::upper_case_acronyms)]
#[napi]
pub enum ContractFunctionType {
    CONSTRUCTOR,
    FUNCTION,
    FALLBACK,
    RECEIVE,
    GETTER,
    MODIFIER,
    FREE_FUNCTION,
}

#[derive(PartialEq)]
pub enum ContractFunctionVisibility {
    Private,
    Internal,
    Public,
    External,
}

pub struct ContractFunction {
    pub name: String,
    pub r#type: ContractFunctionType,
    pub(crate) location: Rc<SourceLocation>,
    pub(crate) contract_name: Option<String>,
    pub(crate) visibility: Option<ContractFunctionVisibility>,
    pub is_payable: Option<bool>,
    /// Fixed up by `Contract.correctSelector`
    pub(crate) selector: RefCell<Option<Vec<u8>>>,
    pub param_types: Option<Vec<Value>>,
}

impl<'a> TryFrom<&'a ContractFunction> for alloy_json_abi::Function {
    type Error = serde_json::Error;

    fn try_from(value: &'a ContractFunction) -> Result<Self, Self::Error> {
        let inputs = value
            .param_types
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(alloy_json_abi::Function {
            name: value.name.clone(),
            inputs,
            outputs: vec![],
            state_mutability: match value.is_payable {
                Some(true) => alloy_json_abi::StateMutability::Payable,
                _ => alloy_json_abi::StateMutability::default(),
            },
        })
    }
}

pub struct CustomError {
    pub selector: [u8; 4],
    pub name: String,
    pub param_types: Vec<Value>,

    def: alloy_json_abi::Error,
}

impl CustomError {
    pub fn from_abi(entry: ContractAbiEntry) -> Result<CustomError, Box<str>> {
        // This is wasteful; to fix that we'd have to implement tighter deserialization
        // for the contract ABI entries.
        let json = serde_json::to_value(&entry).expect("ContractAbiEntry to be round-trippable");

        let selector = edr_solidity::utils::json_abi_error_selector(&json)?;

        Ok(CustomError {
            selector,
            name: entry.name.expect("ABI errors to always have names"),
            param_types: entry.inputs.unwrap_or_default(),
            def: serde_json::from_value(json).map_err(|e| e.to_string().into_boxed_str())?,
        })
    }

    /// Decodes the error data (*with* selector).
    pub fn decode_error_data(
        &self,
        data: &[u8],
    ) -> alloy_dyn_abi::Result<alloy_dyn_abi::DecodedError> {
        self.def.decode_error(data)
    }
}

pub struct Instruction {
    pub pc: u32,
    pub opcode: Opcode,
    pub jump_type: JumpType,
    pub push_data: Option<Vec<u8>>,
    pub(crate) location: Option<Rc<SourceLocation>>,
}

#[derive(Clone, Copy, PartialEq, Eq, strum::IntoStaticStr, strum::Display)]
pub enum JumpType {
    NotJump,
    IntoFunction,
    OutofFunction,
    InternalJump,
}

#[derive(Clone)]
pub struct ImmutableReference {
    pub start: u32,
    pub length: u32,
}

impl From<edr_solidity::artifacts::ImmutableReference> for ImmutableReference {
    fn from(ir: edr_solidity::artifacts::ImmutableReference) -> Self {
        ImmutableReference {
            start: ir.start,
            length: ir.length,
        }
    }
}

#[napi]
pub struct Bytecode {
    pc_to_instruction: HashMap<u32, Instruction>,

    pub(crate) contract: Rc<RefCell<Contract>>,
    #[napi(readonly)]
    pub is_deployment: bool,
    pub(crate) normalized_code: Vec<u8>,
    #[napi(readonly)]
    pub library_address_positions: Vec<u32>,
    pub(crate) immutable_references: Vec<ImmutableReference>,
    #[napi(readonly)]
    pub compiler_version: String,
}

#[napi]
impl Bytecode {
    pub fn new(
        contract: Rc<RefCell<Contract>>,
        is_deployment: bool,
        normalized_code: Vec<u8>,
        instructions: Vec<Instruction>,
        library_address_positions: Vec<u32>,
        immutable_references: Vec<ImmutableReference>,
        compiler_version: String,
    ) -> napi::Result<Bytecode> {
        let mut pc_to_instruction = HashMap::new();
        for inst in instructions {
            pc_to_instruction.insert(inst.pc, inst);
        }

        Ok(Bytecode {
            pc_to_instruction,
            contract,
            is_deployment,
            normalized_code,
            library_address_positions,
            immutable_references,
            compiler_version,
        })
    }

    pub fn get_instruction(&self, pc: u32) -> napi::Result<&Instruction> {
        let instruction = self
            .pc_to_instruction
            .get(&pc)
            .ok_or_else(|| napi::Error::from_reason(format!("Instruction at PC {pc} not found")))?;

        Ok(instruction)
    }

    pub fn has_instruction(&self, pc: u32) -> bool {
        self.pc_to_instruction.contains_key(&pc)
    }
}

#[derive(PartialEq, strum::EnumString)]
#[strum(serialize_all = "camelCase")]
pub enum ContractKind {
    Contract,
    Library,
}

pub struct Contract {
    pub(crate) custom_errors: Vec<CustomError>,
    pub(crate) constructor: Option<Rc<ContractFunction>>,
    pub(crate) fallback: Option<Rc<ContractFunction>>,
    pub(crate) receive: Option<Rc<ContractFunction>>,
    local_functions: Vec<Rc<ContractFunction>>,
    selector_hex_to_function: HashMap<String, Rc<ContractFunction>>,

    pub name: String,
    pub(crate) r#type: ContractKind,
    pub(crate) location: Rc<SourceLocation>,
}

impl Contract {
    pub fn new(
        name: String,
        contract_type: ContractKind,
        location: Rc<SourceLocation>,
    ) -> napi::Result<Contract> {
        Ok(Contract {
            custom_errors: Vec::new(),
            constructor: None,
            fallback: None,
            receive: None,
            local_functions: Vec::new(),
            selector_hex_to_function: HashMap::new(),
            name,
            r#type: contract_type,
            location,
        })
    }

    pub fn add_local_function(&mut self, func: Rc<ContractFunction>) -> napi::Result<()> {
        if matches!(
            func.visibility,
            Some(ContractFunctionVisibility::Public | ContractFunctionVisibility::External)
        ) {
            match func.r#type {
                ContractFunctionType::FUNCTION | ContractFunctionType::GETTER => {
                    let selector = func.selector.try_borrow().expect(
                        "Function selector to be corrected later after creating the source model",
                    );
                    // The original code unwrapped here
                    let selector = selector.as_ref().unwrap();
                    let selector = hex::encode(selector);

                    self.selector_hex_to_function.insert(selector, func.clone());
                }
                ContractFunctionType::CONSTRUCTOR => {
                    self.constructor = Some(func.clone());
                }
                ContractFunctionType::FALLBACK => {
                    self.fallback = Some(func.clone());
                }
                ContractFunctionType::RECEIVE => {
                    self.receive = Some(func.clone());
                }
                _ => {}
            }
        }

        self.local_functions.push(func);

        Ok(())
    }

    pub fn add_custom_error(&mut self, value: CustomError) {
        self.custom_errors.push(value);
    }

    pub fn add_next_linearized_base_contract(
        &mut self,
        base_contract: &Contract,
    ) -> napi::Result<()> {
        if self.fallback.is_none() && base_contract.fallback.is_some() {
            self.fallback.clone_from(&base_contract.fallback);
        }
        if self.receive.is_none() && base_contract.receive.is_some() {
            self.receive.clone_from(&base_contract.receive);
        }

        for base_contract_function in &base_contract.local_functions {
            let base_contract_function_clone = base_contract_function.clone();

            if base_contract_function.r#type != ContractFunctionType::GETTER
                && base_contract_function.r#type != ContractFunctionType::FUNCTION
            {
                continue;
            }

            if base_contract_function.visibility != Some(ContractFunctionVisibility::Public)
                && base_contract_function.visibility != Some(ContractFunctionVisibility::External)
            {
                continue;
            }

            let selector = base_contract_function
                .selector
                .try_borrow()
                .expect("Function selector to be corrected later after creating the source model")
                .clone()
                .unwrap();
            let selector_hex = hex::encode(&*selector);

            self.selector_hex_to_function
                .entry(selector_hex)
                .or_insert(base_contract_function_clone);
        }

        Ok(())
    }

    pub fn get_function_from_selector(&self, selector: &[u8]) -> Option<&Rc<ContractFunction>> {
        let selector_hex = hex::encode(selector);

        self.selector_hex_to_function.get(&selector_hex)
    }

    /// We compute selectors manually, which is particularly hard. We do this
    /// because we need to map selectors to AST nodes, and it seems easier to
    /// start from the AST node. This is surprisingly super hard: things
    /// like inherited enums, structs and ABIv2 complicate it.
    ///
    /// As we know that that can fail, we run a heuristic that tries to correct
    /// incorrect selectors. What it does is checking the
    /// `evm.methodIdentifiers` compiler output, and detect missing
    /// selectors. Then we take those and find contract functions with the
    /// same name. If there are multiple of those we can't do anything. If
    /// there is a single one, it must have an incorrect selector, so we
    /// update it with the `evm.methodIdentifiers`'s value.
    pub fn correct_selector(
        &mut self,
        function_name: String,
        selector: Vec<u8>,
    ) -> napi::Result<bool> {
        let functions = self
            .selector_hex_to_function
            .values()
            .filter(|cf| cf.name == function_name)
            .cloned()
            .collect::<Vec<_>>();

        let function_to_correct = match functions.split_first() {
            Some((function_to_correct, [])) => function_to_correct,
            _ => return Ok(false),
        };

        {
            let mut selector_to_be_corrected = function_to_correct
                .selector
                .try_borrow_mut()
                .expect("Function selector to only be corrected after creating the source model");
            if let Some(selector) = &*selector_to_be_corrected {
                let selector_hex = hex::encode(selector);
                self.selector_hex_to_function.remove(&selector_hex);
            }

            *selector_to_be_corrected = Some(selector.clone());
        }

        let selector_hex = hex::encode(&*selector);
        self.selector_hex_to_function
            .insert(selector_hex, function_to_correct.clone());

        Ok(true)
    }
}
