//! Ported from `hardhat-network/stack-traces/model.ts`.

use std::{cell::OnceCell, collections::HashMap, rc::Rc};

use alloy_dyn_abi::ErrorExt;
use edr_evm::hex;
use edr_solidity::artifacts::ContractAbiEntry;
use napi::{
    bindgen_prelude::{Buffer, ClassInstance, Object, Uint8Array, Undefined},
    Either, Env, JsObject,
};
use napi_derive::napi;
use serde_json::Value;

use super::opcodes::Opcode;
use crate::utils::ClassInstanceRef;

#[napi]
pub struct SourceFile {
    // Referenced because it can be later updated by outside code
    functions: Vec<Rc<ClassInstanceRef<ContractFunction>>>,

    #[napi(readonly)]
    pub source_name: String,
    #[napi(readonly)]
    pub content: String,
}

impl SourceFile {
    pub fn new(source_name: String, content: String) -> napi::Result<SourceFile> {
        Ok(SourceFile {
            functions: Vec::new(),

            content,
            source_name,
        })
    }

    pub fn add_function(&mut self, contract_function: Rc<ClassInstanceRef<ContractFunction>>) {
        self.functions.push(contract_function);
    }

    pub fn get_containing_function(
        &self,
        location: &SourceLocation,
        env: Env,
    ) -> napi::Result<Option<&Rc<ClassInstanceRef<ContractFunction>>>> {
        for func in &self.functions {
            let contains = func
                .borrow(env)?
                .location
                .borrow(env)?
                .contains(location, env);

            if contains {
                return Ok(Some(func));
            }
        }

        Ok(None)
    }
}

#[derive(Clone)]
#[napi]
pub struct SourceLocation {
    line: OnceCell<u32>,
    pub(crate) file: Rc<ClassInstanceRef<SourceFile>>,
    pub offset: u32,
    pub length: u32,
}

#[napi]
impl SourceLocation {
    pub fn new(file: Rc<ClassInstanceRef<SourceFile>>, offset: u32, length: u32) -> SourceLocation {
        SourceLocation {
            line: OnceCell::new(),
            file,
            offset,
            length,
        }
    }

    #[napi(getter, ts_return_type = "SourceFile")]
    pub fn file(&self, env: Env) -> napi::Result<Object> {
        self.file.as_object(env)
    }

    #[napi]
    pub fn get_starting_line_number(&self, env: Env) -> napi::Result<u32> {
        if let Some(line) = self.line.get() {
            return Ok(*line);
        }

        let contents = &self.file.borrow(env)?.content;

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

    // NOTE: This is the actual return type of the function in JS land for now
    #[napi(ts_return_type = "ContractFunction | undefined")]
    pub fn get_containing_function(&self, env: Env) -> napi::Result<Either<JsObject, Undefined>> {
        match self.get_containing_function_inner(env)? {
            Either::A(func) => func.as_object(env).map(Either::A),
            Either::B(()) => Ok(Either::B(())),
        }
    }

    pub fn get_containing_function_inner(
        &self,
        env: Env,
    ) -> napi::Result<Either<Rc<ClassInstanceRef<ContractFunction>>, Undefined>> {
        match self.file.borrow(env)?.get_containing_function(self, env)? {
            Some(func) => Ok(Either::A(func.clone())),
            None => Ok(Either::B(())),
        }
    }

    #[napi]
    pub fn contains(&self, other: &SourceLocation, env: Env) -> bool {
        if !self
            .file
            .ref_equals(&other.file, env)
            .expect("Failed to compare files")
        {
            return false;
        }

        if other.offset < self.offset {
            return false;
        }

        other.offset + other.length <= self.offset + self.length
    }
    #[napi]
    pub fn equals(&self, other: &SourceLocation, env: Env) -> bool {
        self.file
            .ref_equals(&other.file, env)
            .expect("Can't compare references")
            && self.offset == other.offset
            && self.length == other.length
    }
}

#[derive(PartialEq)]
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
#[allow(non_camel_case_types)] // intentionally mimicks the original case in TS
#[allow(clippy::upper_case_acronyms)]
#[napi]
pub enum ContractFunctionVisibility {
    PRIVATE,
    INTERNAL,
    PUBLIC,
    EXTERNAL,
}

#[napi]
pub struct ContractFunction {
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly, js_name = "type")]
    pub r#type: ContractFunctionType,
    pub(crate) location: ClassInstanceRef<SourceLocation>,
    pub(crate) contract: Option<Rc<ClassInstanceRef<Contract>>>,
    #[napi(readonly)]
    pub visibility: Option<ContractFunctionVisibility>,
    #[napi(readonly)]
    pub is_payable: Option<bool>,
    /// Fixed up by `Contract.correctSelector`
    pub(crate) selector: Option<Uint8Array>,
    #[napi(readonly)]
    pub param_types: Option<Vec<Value>>,
}
#[napi]
impl ContractFunction {
    #[napi(getter, ts_return_type = "SourceLocation")]
    pub fn location(&self, env: Env) -> napi::Result<Object> {
        self.location.as_object(env)
    }

    #[napi(getter, ts_return_type = "Contract | undefined")]
    pub fn contract(&self, env: Env) -> napi::Result<Either<Object, Undefined>> {
        match &self.contract {
            Some(contract) => contract.as_object(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }

    pub fn to_alloy(&self) -> Result<alloy_json_abi::Function, Box<str>> {
        let inputs = self
            .param_types
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string().into_boxed_str())?;

        Ok(alloy_json_abi::Function {
            name: self.name.clone(),
            inputs,
            outputs: vec![],
            state_mutability: match self.is_payable {
                Some(true) => alloy_json_abi::StateMutability::Payable,
                _ => alloy_json_abi::StateMutability::default(),
            },
        })
    }
}

#[napi]
pub struct CustomError {
    #[napi(readonly)]
    pub selector: Uint8Array,
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly)]
    pub param_types: Vec<Value>,

    def: alloy_json_abi::Error,
}

#[napi]
impl CustomError {
    pub fn from_abi(entry: ContractAbiEntry) -> Result<CustomError, Box<str>> {
        // This is wasteful; to fix that we'd have to implement tighter deserialization
        // for the contract ABI entries.
        let json = serde_json::to_value(&entry).expect("ContractAbiEntry to be round-trippable");

        let selector = edr_solidity::utils::json_abi_error_selector(&json)?;

        Ok(CustomError {
            selector: Uint8Array::from(&selector),
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

#[napi]
pub struct Instruction {
    #[napi(readonly)]
    pub pc: u32,
    #[napi(readonly, ts_type = "opcodes.Opcode")]
    pub opcode: Opcode,
    #[napi(readonly)]
    pub jump_type: JumpType,
    #[napi(readonly)]
    pub push_data: Option<Buffer>,
    pub(crate) location: Option<ClassInstanceRef<SourceLocation>>,
}

#[derive(strum::IntoStaticStr, PartialEq)]
#[allow(non_camel_case_types)] // intentionally mimicks the original case in TS
#[napi]
pub enum JumpType {
    NOT_JUMP,
    INTO_FUNCTION,
    OUTOF_FUNCTION,
    INTERNAL_JUMP,
}

#[napi]
pub fn jump_type_to_string(jump_type: JumpType) -> &'static str {
    jump_type.into()
}

#[derive(Clone)]
#[napi(object)]
pub struct ImmutableReference {
    #[napi(readonly)]
    pub start: u32,
    #[napi(readonly)]
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
impl Instruction {
    pub fn new(
        pc: u32,
        opcode: Opcode,
        jump_type: JumpType,
        push_data: Option<Buffer>,
        location: Option<ClassInstanceRef<SourceLocation>>,
    ) -> napi::Result<Instruction> {
        Ok(Instruction {
            pc,
            opcode,
            jump_type,
            push_data,
            location,
        })
    }

    #[napi(getter, ts_return_type = "SourceLocation | undefined")]
    pub fn location(&self, env: Env) -> napi::Result<Either<Object, Undefined>> {
        match &self.location {
            Some(loc) => loc.as_object(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }
}

#[napi]
pub struct Bytecode {
    pc_to_instruction: HashMap<u32, ClassInstanceRef<Instruction>>,

    pub(crate) contract: Rc<ClassInstanceRef<Contract>>,
    #[napi(readonly)]
    pub is_deployment: bool,
    #[napi(readonly)]
    pub normalized_code: Buffer,
    #[napi(readonly)]
    pub library_address_positions: Vec<u32>,
    #[napi(readonly)]
    pub immutable_references: Vec<ImmutableReference>,
    #[napi(readonly)]
    pub compiler_version: String,
}

#[napi]
impl Bytecode {
    #[allow(clippy::too_many_arguments)] // mimick the original code
    pub fn new(
        contract: Rc<ClassInstanceRef<Contract>>,
        is_deployment: bool,
        normalized_code: Buffer,
        instructions: Vec<ClassInstance<Instruction>>,
        library_address_positions: Vec<u32>,
        immutable_references: Vec<ImmutableReference>,
        compiler_version: String,
        env: Env,
    ) -> napi::Result<Bytecode> {
        let mut pc_to_instruction = HashMap::new();
        for inst in instructions {
            let pc = inst.pc;
            let inst = ClassInstanceRef::from_obj(inst, env)?;

            pc_to_instruction.insert(pc, inst);
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

    #[napi(ts_return_type = "Instruction")]
    pub fn get_instruction(&self, pc: u32, env: Env) -> napi::Result<Object> {
        self.get_instruction_inner(pc)?.as_object(env)
    }

    pub fn get_instruction_inner(&self, pc: u32) -> napi::Result<&ClassInstanceRef<Instruction>> {
        let instruction = self
            .pc_to_instruction
            .get(&pc)
            .ok_or_else(|| napi::Error::from_reason(format!("Instruction at PC {pc} not found")))?;

        Ok(instruction)
    }

    #[napi]
    pub fn has_instruction(&self, pc: u32) -> bool {
        self.pc_to_instruction.contains_key(&pc)
    }

    #[napi(getter, ts_return_type = "Contract")]
    pub fn contract(&self, env: Env) -> napi::Result<Object> {
        self.contract.as_object(env)
    }
}

#[allow(non_camel_case_types)] // intentionally mimicks the original case in TS
#[allow(clippy::upper_case_acronyms)]
#[napi]
#[derive(PartialEq)]
pub enum ContractType {
    CONTRACT,
    LIBRARY,
}

#[napi]
pub struct Contract {
    pub(crate) custom_errors: Vec<ClassInstanceRef<CustomError>>,
    pub(crate) constructor: Option<Rc<ClassInstanceRef<ContractFunction>>>,
    pub(crate) fallback: Option<Rc<ClassInstanceRef<ContractFunction>>>,
    pub(crate) receive: Option<Rc<ClassInstanceRef<ContractFunction>>>,
    local_functions: Vec<Rc<ClassInstanceRef<ContractFunction>>>,
    selector_hex_to_function: HashMap<String, Rc<ClassInstanceRef<ContractFunction>>>,

    #[napi(readonly)]
    pub name: String,
    #[napi(readonly, js_name = "type")]
    pub r#type: ContractType,
    pub(crate) location: ClassInstanceRef<SourceLocation>,
}

#[napi]
impl Contract {
    pub fn new(
        name: String,
        contract_type: ContractType,
        location: ClassInstanceRef<SourceLocation>,
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

    #[napi(getter, ts_return_type = "SourceLocation")]
    pub fn location(&self, env: Env) -> napi::Result<Object> {
        self.location.as_object(env)
    }

    #[napi(getter, ts_return_type = "Array<CustomError>")]
    pub fn custom_errors(&self, env: Env) -> napi::Result<Vec<Object>> {
        self.custom_errors
            .iter()
            .map(|value| value.as_object(env))
            .collect()
    }

    #[napi(getter, ts_return_type = "ContractFunction | undefined")]
    pub fn constructor_function(&self, env: Env) -> napi::Result<Either<Object, Undefined>> {
        match &self.constructor {
            Some(a) => a.as_object(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }

    #[napi(getter, ts_return_type = "ContractFunction | undefined")]
    pub fn fallback(&self, env: Env) -> napi::Result<Either<Object, Undefined>> {
        match &self.fallback {
            Some(a) => a.as_object(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }
    #[napi(getter, ts_return_type = "ContractFunction | undefined")]
    pub fn receive(&self, env: Env) -> napi::Result<Either<Object, Undefined>> {
        match &self.receive {
            Some(a) => a.as_object(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }

    pub fn add_local_function(
        &mut self,
        func_ref: Rc<ClassInstanceRef<ContractFunction>>,
        env: Env,
    ) -> napi::Result<()> {
        let func = func_ref.borrow(env)?;

        if matches!(
            func.visibility,
            Some(ContractFunctionVisibility::PUBLIC | ContractFunctionVisibility::EXTERNAL)
        ) {
            match func.r#type {
                ContractFunctionType::FUNCTION | ContractFunctionType::GETTER => {
                    // The original code unwrapped here
                    let selector = func.selector.as_ref().unwrap();
                    let selector = hex::encode(selector);

                    self.selector_hex_to_function
                        .insert(selector, func_ref.clone());
                }
                ContractFunctionType::CONSTRUCTOR => {
                    self.constructor = Some(func_ref.clone());
                }
                ContractFunctionType::FALLBACK => {
                    self.fallback = Some(func_ref.clone());
                }
                ContractFunctionType::RECEIVE => {
                    self.receive = Some(func_ref.clone());
                }
                _ => {}
            }
        }

        drop(func);
        self.local_functions.push(func_ref);

        Ok(())
    }

    pub fn add_custom_error(&mut self, value: ClassInstanceRef<CustomError>) {
        self.custom_errors.push(value);
    }

    pub fn add_next_linearized_base_contract(
        &mut self,
        base_contract: &Contract,
        env: Env,
    ) -> napi::Result<()> {
        if self.fallback.is_none() && base_contract.fallback.is_some() {
            self.fallback.clone_from(&base_contract.fallback);
        }
        if self.receive.is_none() && base_contract.receive.is_some() {
            self.receive.clone_from(&base_contract.receive);
        }

        for base_contract_function in &base_contract.local_functions {
            let base_contract_function_clone = base_contract_function.clone();
            let base_contract_function = base_contract_function.borrow(env)?;

            if base_contract_function.r#type != ContractFunctionType::GETTER
                && base_contract_function.r#type != ContractFunctionType::FUNCTION
            {
                continue;
            }

            if base_contract_function.visibility != Some(ContractFunctionVisibility::PUBLIC)
                && base_contract_function.visibility != Some(ContractFunctionVisibility::EXTERNAL)
            {
                continue;
            }

            let selector = base_contract_function.selector.clone().unwrap();
            let selector_hex = hex::encode(&*selector);

            self.selector_hex_to_function
                .entry(selector_hex)
                .or_insert(base_contract_function_clone);
        }

        Ok(())
    }

    #[napi(ts_return_type = "ContractFunction | undefined")]
    pub fn get_function_from_selector(
        &self,
        selector: Uint8Array,
        env: Env,
    ) -> napi::Result<Either<JsObject, Undefined>> {
        match self.get_function_from_selector_inner(selector.as_ref()) {
            Some(func) => func.as_object(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }

    pub fn get_function_from_selector_inner(
        &self,
        selector: &[u8],
    ) -> Option<&Rc<ClassInstanceRef<ContractFunction>>> {
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
        selector: Uint8Array,
        env: Env,
    ) -> napi::Result<bool> {
        let functions = self
            .selector_hex_to_function
            .values()
            .filter_map(|cf| match cf.borrow(env).map(|x| x.name == function_name) {
                Ok(true) => Some(Ok(cf.clone())),
                Ok(false) => None,
                Err(e) => Some(Err(e)),
            })
            .collect::<napi::Result<Vec<_>>>()?;

        let function_to_correct = match functions.split_first() {
            Some((function_to_correct, [])) => function_to_correct,
            _ => return Ok(false),
        };

        {
            let mut instance = function_to_correct.borrow_mut(env)?;
            if let Some(selector) = &instance.selector {
                let selector_hex = hex::encode(selector);
                self.selector_hex_to_function.remove(&selector_hex);
            }

            instance.selector = Some(selector.clone());
        }

        let selector_hex = hex::encode(&*selector);
        self.selector_hex_to_function
            .insert(selector_hex, function_to_correct.clone());

        Ok(true)
    }
}
