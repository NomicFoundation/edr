use std::{cell::OnceCell, rc::Rc};

use napi::{
    bindgen_prelude::{ClassInstance, Object, Uint8Array, Undefined},
    Either, Env, JsObject,
};
use napi_derive::napi;
use serde_json::{json, Value};

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

    fn location(&self, env: Env) -> napi::Result<ClassInstance<SourceLocation>> {
        let obj = self.as_inner(env)?;
        obj.get_named_property::<ClassInstance<SourceLocation>>("location")
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

    // TODO: See if this even works
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
    /// TODO: Replace with `Contract`
    #[napi(readonly, ts_type = "any")]
    pub contract: Object,
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

// WIP area below:
use napi::bindgen_prelude::Buffer;

#[napi]
pub enum JumpType {
    NotJump,
    IntoFunction,
    OutOfFunction,
    InternalJump,
}

#[napi]
pub enum Opcode {
    // Only listing the opcodes that are used in the stack tracing logic
    Stop = 0x00,

    Iszero = 0x15,
    Codesize = 0x38,
    Extcodesize = 0x3b,

    Jump = 0x56,
    Jumpi = 0x57,
    Jumpdest = 0x5b,

    Push1 = 0x60,
    //...
    Push32 = 0x7f,

    Create = 0xf0,
    Call = 0xf1,
    Callcode = 0xf2,
    Return = 0xf3,
    Delegatecall = 0xf4,
    Create2 = 0xf5,

    Staticcall = 0xfa,

    Revert = 0xfd,
    Invalid = 0xfe,
    Selfdestruct = 0xFF,
}

#[derive(Clone)]
#[napi(object)]
pub struct Instruction {
    #[napi(readonly)]
    pub pc: u32,
    // Should be an enum but TypeScript type system does not follow structural
    // typing for enums, so we can't define our own type and we use a number instead.
    #[napi(readonly)]
    pub opcode: u8,
    // Should be an enum but TypeScript type system does not follow structural
    // typing for enums, so we can't define our own type and we use a number instead.
    #[napi(readonly)]
    pub jump_type: u8,
    #[napi(readonly)]
    pub push_data: Option<Buffer>,
    #[napi(readonly)]
    pub location: Option<Value>,
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
pub struct Bytecode {
    // Emit a fake field to appease TypeScript's type system to be backwards
    // compatible with the existing `Bytecode` interface.
    // Originally, this property is marked as `private` but napi-rs does not
    // support private fields and we can't use ES6 `#`-private fields because
    // it's also considered incompatible by the TypeScript compiler.
    /// Internal field, do not use.
    #[napi(readonly, js_name = "_pcToInstruction", ts_type = "any")]
    pub _appease_typescript: (),

    #[napi(readonly)]
    pub contract: Value,
    #[napi(readonly)]
    pub is_deployment: bool,
    #[napi(readonly)]
    pub normalized_code: Buffer,
    #[napi(readonly)]
    pub instructions: Vec<Instruction>,
    #[napi(readonly)]
    pub library_address_positions: Vec<u32>,
    #[napi(readonly)]
    pub immutable_references: Vec<ImmutableReference>,
    #[napi(readonly)]
    pub compiler_version: String,
}

#[napi]
impl Bytecode {
    #[napi]
    pub fn get_instruction(&self, pc: u32) -> napi::Result<Instruction> {
        self.instructions
            .get(pc as usize)
            .cloned()
            .ok_or_else(|| napi::Error::from_reason(format!("Instruction at PC {} not found", pc)))
    }

    #[napi]
    pub fn has_instruction(&self, pc: u32) -> bool {
        self.instructions.get(pc as usize).is_some()
    }
}
