use std::{cell::OnceCell, collections::HashMap, rc::Rc};

use edr_evm::hex;
use napi::{
    bindgen_prelude::{Buffer, ClassInstance, FromNapiValue, Object, This, Uint8Array, Undefined},
    Either, Env, JsObject,
};
use napi_derive::napi;
use serde_json::{json, Value};

use super::opcodes::Opcode;

const ENABLE_DEBUG: bool = false;

macro_rules! neprintln {
    ($($arg:tt)*) => {
        if ENABLE_DEBUG {
            eprintln!("{}", format_args!($($arg)*))
        }
    };
    () => {
    };
}

#[derive(Clone)]
struct ContractFunctionRef {
    r#ref: Rc<napi::Ref<()>>,
}

impl ContractFunctionRef {
    fn from_obj(obj: Object, env: Env) -> napi::Result<ContractFunctionRef> {
        let r#ref = env.create_reference(obj)?;
        let r#ref = Rc::new(r#ref);
        Ok(ContractFunctionRef { r#ref })
    }

    fn as_inner(&self, env: Env) -> napi::Result<Object> {
        env.get_reference_value::<Object>(&self.r#ref)
    }

    fn name(&self, env: Env) -> napi::Result<String> {
        let obj = self.as_inner(env)?;
        obj.get_named_property::<String>("name")
    }

    fn location(&self, env: Env) -> napi::Result<ClassInstance<SourceLocation>> {
        let obj = self.as_inner(env)?;
        obj.get_named_property::<ClassInstance<SourceLocation>>("location")
    }

    fn r#type(&self, env: Env) -> napi::Result<ContractFunctionType> {
        let obj = self.as_inner(env)?;
        obj.get_named_property::<ContractFunctionType>("type")
    }

    fn visibility(&self, env: Env) -> napi::Result<Option<ContractFunctionVisibility>> {
        let obj = self.as_inner(env)?;
        obj.get_named_property::<Option<ContractFunctionVisibility>>("visibility")
    }

    fn selector(&self, env: Env) -> napi::Result<Option<Uint8Array>> {
        let obj = self.as_inner(env)?;
        obj.get_named_property::<Option<Uint8Array>>("selector")
    }

    fn set_selector(&self, selector: Uint8Array, env: Env) -> napi::Result<()> {
        let mut obj = self.as_inner(env)?;
        obj.set_named_property("selector", selector)
    }
}

#[napi]
pub struct SourceFile {
    // Referenced because it can be later updated by outside code
    functions: Vec<ContractFunctionRef>,

    #[napi(readonly)]
    pub source_name: String,
    #[napi(readonly)]
    pub content: String,
}

#[napi]
impl SourceFile {
    #[napi(constructor)]
    pub fn new(source_name: String, content: String) -> napi::Result<SourceFile> {
        neprintln!("SourceFile::new in Rust");
        Ok(SourceFile {
            functions: Vec::new(),

            content,
            source_name,
        })
    }

    #[napi]
    pub fn add_function(&mut self, contract_function: JsObject, env: Env) -> napi::Result<()> {
        neprintln!("SourceFile::add_function in Rust");
        let contract_function = ContractFunctionRef::from_obj(contract_function, env)?;

        self.functions.push(contract_function);
        Ok(())
    }

    #[napi]
    pub fn get_containing_function(
        &self,
        location: &SourceLocation,
        env: Env,
    ) -> napi::Result<Either<JsObject, Undefined>> {
        neprintln!("SourceFile::get_containing_function in Rust");

        for func in &self.functions {
            // This is actually calling our own method but we only have a handle
            // to JsObject, so first let's see if it works and then make sure
            // it works without crossing the JS side redundantly.
            let func_location = func.location(env)?;
            let contains = func_location.contains(&location, env);
            neprintln!("Contains: {:?}", contains);

            if contains {
                return Ok(Either::A(func.as_inner(env)?));
            }
        }

        return Ok(Either::B(()));
    }
}

#[derive(Clone)]
/// Wraps the original `SourceFile` class instance.
struct SourceFileRef {
    r#ref: Rc<napi::Ref<()>>,
    instance: Rc<ClassInstance<SourceFile>>,
}

impl SourceFileRef {
    /// Creates a reference from the external `Object`.
    fn from_obj(instance: ClassInstance<SourceFile>, env: Env) -> napi::Result<SourceFileRef> {
        let obj = instance.as_object(env);
        let r#ref = env.create_reference(obj)?;

        Ok(SourceFileRef {
            r#ref: Rc::new(r#ref),
            instance: Rc::new(instance),
        })
    }

    /// Returns the inner `Object` from the reference.
    fn as_inner(&self, env: Env) -> napi::Result<Object> {
        // NOTE: It's important to return the original object rather than the
        // one from `ClassInstance::as_object`
        env.get_reference_value::<Object>(&self.r#ref)
    }

    fn equals(&self, other: &SourceFileRef, env: Env) -> napi::Result<bool> {
        neprintln!("SourceFileRef::equals in Rust");
        let obj = self.as_inner(env)?;
        let other_obj = other.as_inner(env)?;

        env.strict_equals(obj, other_obj)
    }
}

#[derive(Clone)]
#[napi]
pub struct SourceLocation {
    line: OnceCell<u32>,
    file: SourceFileRef,
    pub offset: u32,
    pub length: u32,
}

#[napi]
impl SourceLocation {
    #[napi(constructor)]
    pub fn new(
        file: ClassInstance<SourceFile>,
        offset: u32,
        length: u32,
        env: Env,
    ) -> napi::Result<SourceLocation> {
        neprintln!("SourceLocation::new in Rust");
        Ok(SourceLocation {
            line: OnceCell::new(),
            file: SourceFileRef::from_obj(file, env)?,
            offset,
            length,
        })
    }

    // It's impossible to have a `Reference` be a property as it's not supported
    // by napi-rs, so we use a getter, instead
    #[napi(getter, ts_return_type = "SourceFile")]
    pub fn file(&self, env: Env) -> napi::Result<JsObject> {
        neprintln!("SourceLocation::file in Rust");
        self.file.as_inner(env)
    }

    #[napi]
    pub fn get_starting_line_number(&self) -> napi::Result<u32> {
        if let Some(line) = self.line.get() {
            return Ok(*line);
        }

        let contents = &self.file.instance.content;

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
        neprintln!("SourceLocation::get_containing_function in Rust");

        self.file.instance.get_containing_function(self, env)
    }

    #[napi]
    pub fn contains(&self, other: &SourceLocation, env: Env) -> bool {
        neprintln!("SourceLocation::contains in Rust");
        if !self
            .file
            .equals(&other.file, env)
            .expect("Failed to compare files")
        {
            return false;
        }

        if other.offset < self.offset {
            return false;
        }

        return other.offset + other.length <= self.offset + self.length;
    }
    #[napi]
    pub fn equals(&self, other: &SourceLocation, env: Env) -> bool {
        neprintln!("SourceLocation::equals in Rust");
        self.file.equals(&other.file, env).expect("TODO")
            && self.offset == other.offset
            && self.length == other.length
    }
}

#[derive(PartialEq)]
#[allow(non_camel_case_types)] // intentionally mimicks the original case in TS
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
#[napi]
pub enum ContractFunctionVisibility {
    PRIVATE,
    INTERNAL,
    PUBLIC,
    EXTERNAL,
}

#[napi(object)]
pub struct ContractFunction {
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly, js_name = "type")]
    pub r#type: ContractFunctionType,
    #[napi(readonly)]
    pub location: ClassInstance<SourceLocation>,
    #[napi(readonly)]
    pub contract: ClassInstance<Contract>,
    #[napi(readonly)]
    pub visibility: Option<ContractFunctionVisibility>,
    #[napi(readonly)]
    pub is_payable: Option<bool>,
    /// Fixed up by `Contract.correctSelector`
    pub selector: Option<Uint8Array>,
    #[napi(readonly)]
    pub param_types: Option<Vec<Value>>,
}

#[napi]
pub struct CustomError {
    #[napi(readonly)]
    pub selector: Uint8Array,
    #[napi(readonly)]
    pub name: String,
    #[napi(readonly)]
    pub param_types: Vec<Value>,
}

#[napi]
impl CustomError {
    #[napi(js_name = "fromABI")]
    pub fn from_abi(name: String, inputs: Vec<Value>) -> Either<CustomError, Undefined> {
        let selector = edr_solidity::utils::json_abi_error_selector(&json!({
          "name": name,
          "inputs": inputs
        }));
        let selector = match selector {
            Ok(selector) => selector,
            Err(_) => return Either::B(()),
        };

        Either::A(CustomError {
            selector: Uint8Array::from(&selector),
            name,
            param_types: inputs,
        })
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
    // #[napi(readonly, ts_type = "SourceLocation | undefined")]
    location: Option<napi::Ref<()>>,
}

#[allow(non_camel_case_types)] // intentionally mimicks the original case in TS
#[napi]
// TODO: Disable `const enum` selectively for this one
pub enum JumpType {
    NOT_JUMP,
    INTO_FUNCTION,
    OUTOF_FUNCTION,
    INTERNAL_JUMP,
}

#[derive(Clone)]
#[napi(object)]
pub struct ImmutableReference {
    #[napi(readonly)]
    pub start: u32,
    #[napi(readonly)]
    pub length: u32,
}

#[napi]
impl Instruction {
    #[napi(constructor)]
    pub fn new(
        pc: u32,
        #[napi(ts_arg_type = "opcodes.Opcode")] opcode: Opcode,
        jump_type: JumpType,
        push_data: Option<Buffer>,
        location: Option<ClassInstance<SourceLocation>>,
        env: Env,
    ) -> napi::Result<Instruction> {
        let loc_ref = match location {
            Some(loc) => {
                let loc_ref = env.create_reference(loc)?;
                Some(loc_ref)
            }
            None => None,
        };

        Ok(Instruction {
            pc,
            opcode,
            jump_type,
            push_data,
            location: loc_ref,
        })
    }

    #[napi(getter, ts_return_type = "SourceLocation | undefined")]
    pub fn location(&self, env: Env) -> napi::Result<Either<Object, Undefined>> {
        match &self.location {
            Some(loc) => {
                let loc = env.get_reference_value::<Object>(loc)?;
                Ok(Either::A(loc))
            }
            None => Ok(Either::B(())),
        }
    }
}

#[napi]
pub struct Bytecode {
    pc_to_instruction: HashMap<u32, napi::Ref<()>>,

    contract: napi::Ref<()>,
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
    #[napi(constructor)]
    pub fn new(
        contract: ClassInstance<Contract>,
        is_deployment: bool,
        normalized_code: Buffer,
        instructions: Vec<ClassInstance<Instruction>>,
        library_address_positions: Vec<u32>,
        immutable_references: Vec<ImmutableReference>,
        compiler_version: String,
        env: Env,
    ) -> napi::Result<Bytecode> {
        let contract_ref = env.create_reference(contract)?;

        let mut pc_to_instruction = HashMap::new();
        for inst in instructions {
            let pc = inst.pc;
            let inst = inst.as_object(env);
            let r#ref = env.create_reference(inst)?;

            pc_to_instruction.insert(pc, r#ref);
        }

        Ok(Bytecode {
            pc_to_instruction,
            contract: contract_ref,
            is_deployment,
            normalized_code,
            library_address_positions,
            immutable_references,
            compiler_version,
        })
    }

    #[napi(ts_return_type = "Instruction")]
    pub fn get_instruction(&self, pc: u32, env: Env) -> napi::Result<Object> {
        let obj = self.pc_to_instruction.get(&pc).ok_or_else(|| {
            napi::Error::from_reason(format!("Instruction at PC {} not found", pc))
        })?;

        Ok(env.get_reference_value::<Object>(obj)?)
    }

    #[napi]
    pub fn has_instruction(&self, pc: u32) -> bool {
        self.pc_to_instruction.contains_key(&pc)
    }

    // TODO: Change the types to Contract once we port it
    #[napi(getter, ts_return_type = "Contract")]
    pub fn contract(&self, env: Env) -> napi::Result<Object> {
        env.get_reference_value::<Object>(&self.contract)
    }
}

#[allow(non_camel_case_types)] // intentionally mimicks the original case in TS
#[napi]
pub enum ContractType {
    CONTRACT,
    LIBRARY,
}

#[napi]
pub struct Contract {
    custom_errors: Vec<napi::Ref<()>>,
    constructor: Option<ContractFunctionRef>,
    fallback: Option<ContractFunctionRef>,
    receive: Option<ContractFunctionRef>,
    local_functions: Vec<ContractFunctionRef>,
    selector_hex_to_function: HashMap<String, ContractFunctionRef>,

    #[napi(readonly)]
    pub name: String,
    #[napi(readonly, js_name = "type")]
    pub r#type: ContractType,
    location: napi::Ref<()>, /* SourceLocation */
}

#[napi]
impl Contract {
    #[napi(constructor)]
    pub fn new(
        name: String,
        contract_type: ContractType,
        location: ClassInstance<SourceLocation>,
        env: Env,
    ) -> napi::Result<Contract> {
        let location_obj = location.as_object(env);
        let location_ref = env.create_reference(location_obj)?;

        Ok(Contract {
            custom_errors: Vec::new(),
            constructor: None,
            fallback: None,
            receive: None,
            local_functions: Vec::new(),
            selector_hex_to_function: HashMap::new(),
            name,
            r#type: contract_type,
            location: location_ref,
        })
    }

    #[napi(getter, ts_return_type = "SourceLocation")]
    pub fn location(&self, env: Env) -> napi::Result<Object> {
        env.get_reference_value::<Object>(&self.location)
    }

    #[napi(getter, ts_return_type = "CustomError[]")]
    pub fn custom_errors(&self, env: Env) -> napi::Result<Vec<Object>> {
        self.custom_errors
            .iter()
            .map(|r#ref| env.get_reference_value::<Object>(r#ref))
            .collect()
    }

    #[napi(getter, ts_return_type = "ContractFunction | undefined")]
    pub fn constructor_function(&self, env: Env) -> napi::Result<Either<Object, Undefined>> {
        match &self.constructor {
            Some(a) => a.as_inner(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }

    #[napi(getter, ts_return_type = "ContractFunction | undefined")]
    pub fn fallback(&self, env: Env) -> napi::Result<Either<Object, Undefined>> {
        match &self.fallback {
            Some(a) => a.as_inner(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }
    #[napi(getter, ts_return_type = "ContractFunction | undefined")]
    pub fn receive(&self, env: Env) -> napi::Result<Either<Object, Undefined>> {
        match &self.receive {
            Some(a) => a.as_inner(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }

    #[napi]
    pub fn add_local_function(
        &mut self,
        func: JsObject,
        this: This<JsObject>,
        env: Env,
    ) -> napi::Result<()> {
        let func_contract = func.get_named_property::<Object>("contract")?;
        if !env.strict_equals(this, &func_contract)? {
            return Err(napi::Error::from_reason("Function isn't local"));
        }

        let r#ref = ContractFunctionRef::from_obj(func, env)?;
        let func = ContractFunction::from_unknown(r#ref.as_inner(env)?.into_unknown())?;

        if matches!(
            func.visibility,
            Some(ContractFunctionVisibility::PUBLIC | ContractFunctionVisibility::EXTERNAL)
        ) {
            match func.r#type {
                ContractFunctionType::FUNCTION | ContractFunctionType::GETTER => {
                    // The original code unwrapped here
                    let selector = func.selector.as_ref().unwrap();
                    let selector = hex::encode(&*selector);

                    self.selector_hex_to_function
                        .insert(selector, r#ref.clone());
                }
                ContractFunctionType::CONSTRUCTOR => {
                    self.constructor = Some(r#ref.clone());
                }
                ContractFunctionType::FALLBACK => {
                    self.fallback = Some(r#ref.clone());
                }
                ContractFunctionType::RECEIVE => {
                    self.receive = Some(r#ref.clone());
                }
                _ => {}
            }
        }

        self.local_functions.push(r#ref);

        Ok(())
    }

    #[napi]
    pub fn add_custom_error(
        &mut self,
        custom_error: ClassInstance<CustomError>,
        env: Env,
    ) -> napi::Result<()> {
        let r#ref = env.create_reference(custom_error)?;
        self.custom_errors.push(r#ref);
        Ok(())
    }

    #[napi]
    pub fn add_next_linearized_base_contract(
        &mut self,
        base_contract: ClassInstance<Contract>,
        env: Env,
    ) -> napi::Result<()> {
        if self.fallback.is_none() && base_contract.fallback.is_some() {
            self.fallback = base_contract.fallback.clone();
        }
        if self.receive.is_none() && base_contract.receive.is_some() {
            self.receive = base_contract.receive.clone();
        }

        for base_contract_function in &base_contract.local_functions {
            if base_contract_function.r#type(env)? != ContractFunctionType::GETTER
                && base_contract_function.r#type(env)? != ContractFunctionType::FUNCTION
            {
                continue;
            }

            if base_contract_function.visibility(env)? != Some(ContractFunctionVisibility::PUBLIC)
                && base_contract_function.visibility(env)?
                    != Some(ContractFunctionVisibility::EXTERNAL)
            {
                continue;
            }

            let selector = base_contract_function.selector(env)?.clone().unwrap();
            let selector_hex = hex::encode(&*selector);

            if !self.selector_hex_to_function.contains_key(&selector_hex) {
                self.selector_hex_to_function
                    .insert(selector_hex, base_contract_function.clone());
            }
        }

        Ok(())
    }

    #[napi(ts_return_type = "ContractFunction | undefined")]
    pub fn get_function_from_selector(
        &self,
        selector: Uint8Array,
        env: Env,
    ) -> napi::Result<Either<JsObject, Undefined>> {
        let selector_hex = hex::encode(&*selector);

        match self.selector_hex_to_function.get(&selector_hex) {
            Some(func) => func.as_inner(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }

    /**
     * We compute selectors manually, which is particularly hard. We do this
     * because we need to map selectors to AST nodes, and it seems easier to start
     * from the AST node. This is surprisingly super hard: things like inherited
     * enums, structs and ABIv2 complicate it.
     *
     * As we know that that can fail, we run a heuristic that tries to correct
     * incorrect selectors. What it does is checking the `evm.methodIdentifiers`
     * compiler output, and detect missing selectors. Then we take those and
     * find contract functions with the same name. If there are multiple of those
     * we can't do anything. If there is a single one, it must have an incorrect
     * selector, so we update it with the `evm.methodIdentifiers`'s value.
     */
    #[napi]
    pub fn correct_selector(
        &mut self,
        function_name: String,
        selector: Uint8Array,
        env: Env,
    ) -> napi::Result<bool> {
        let functions = self
            .selector_hex_to_function
            .values()
            .filter_map(|cf| match cf.name(env) {
                Ok(name) if name == function_name => Some(Ok(cf.clone())),
                Ok(_) => None,
                Err(e) => Some(Err(e)),
            })
            .collect::<napi::Result<Vec<ContractFunctionRef>>>()?;

        let function_to_correct = match functions.split_first() {
            Some((function_to_correct, [])) => function_to_correct,
            _ => return Ok(false),
        };

        if let Some(selector) = &function_to_correct.selector(env)? {
            let selector_hex = hex::encode(&*selector);
            self.selector_hex_to_function.remove(&selector_hex);
        }

        function_to_correct.set_selector(selector.clone(), env)?;
        let selector_hex = hex::encode(&*selector);
        self.selector_hex_to_function
            .insert(selector_hex, function_to_correct.clone());

        Ok(true)
    }
}
