use std::rc::Rc;

use edr_solidity::{
    artifacts::BuildInfo,
    build_model::{Bytecode, ContractFunctionType},
    compiler::create_models_and_decode_bytecodes,
    contracts_identifier::ContractsIdentifier,
};
use napi::{
    bindgen_prelude::{ClassInstance, Either3, Either4, Uint8Array, Undefined},
    Either, Env,
};
use napi_derive::napi;
use serde::{Deserialize, Serialize};

use super::{
    message_trace::{CallMessageTrace, CreateMessageTrace, PrecompileMessageTrace},
    solidity_stack_trace::{
        FALLBACK_FUNCTION_NAME, RECEIVE_FUNCTION_NAME, UNRECOGNIZED_CONTRACT_NAME,
        UNRECOGNIZED_FUNCTION_NAME,
    },
};
use crate::trace::model::BytecodeWrapper;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingConfig {
    pub build_infos: Option<Vec<BuildInfo>>,
    pub ignore_contracts: Option<bool>,
}

#[derive(Default)]
#[napi]
pub struct VmTraceDecoder {
    contracts_identifier: ContractsIdentifier,
}

#[napi]
impl VmTraceDecoder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[napi(catch_unwind)]
    pub fn add_bytecode(&mut self, bytecode: ClassInstance<BytecodeWrapper>) {
        self.add_bytecode_inner(bytecode.inner().clone());
    }

    pub fn add_bytecode_inner(&mut self, bytecode: Rc<Bytecode>) {
        self.contracts_identifier.add_bytecode(bytecode);
    }

    #[napi(catch_unwind)]
    pub fn try_to_decode_message_trace(
        &mut self,
        message_trace: Either3<PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<Either3<PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace>> {
        match message_trace {
            precompile @ Either3::A(..) => Ok(precompile),
            // NOTE: The branches below are the same with the difference of `is_create`
            Either3::B(mut call) => {
                let is_create = false;

                let bytecode = self
                    .contracts_identifier
                    .get_bytecode_for_call(call.code.as_ref(), is_create);

                let steps: Vec<_> = call
                    .steps
                    .into_iter()
                    .map(|step| {
                        let trace = match step {
                            Either4::A(step) => return Ok(Either4::A(step)),
                            Either4::B(precompile) => Either3::A(precompile),
                            Either4::C(create) => Either3::B(create),
                            Either4::D(call) => Either3::C(call),
                        };

                        Ok(match self.try_to_decode_message_trace(trace, env)? {
                            Either3::A(precompile) => Either4::B(precompile),
                            Either3::B(create) => Either4::C(create),
                            Either3::C(call) => Either4::D(call),
                        })
                    })
                    .collect::<napi::Result<_>>()?;

                let bytecode = bytecode
                    .map(|b| BytecodeWrapper::new(b).into_instance(env))
                    .transpose()?;

                call.bytecode = bytecode;
                call.steps = steps;

                Ok(Either3::B(call))
            }
            Either3::C(mut create @ CreateMessageTrace { .. }) => {
                let is_create = true;

                let bytecode = self
                    .contracts_identifier
                    .get_bytecode_for_call(create.code.as_ref(), is_create);

                let steps: Vec<_> = create
                    .steps
                    .into_iter()
                    .map(|step| {
                        let trace = match step {
                            Either4::A(step) => return Ok(Either4::A(step)),
                            Either4::B(precompile) => Either3::A(precompile),
                            Either4::C(create) => Either3::B(create),
                            Either4::D(call) => Either3::C(call),
                        };

                        Ok(match self.try_to_decode_message_trace(trace, env)? {
                            Either3::A(precompile) => Either4::B(precompile),
                            Either3::B(create) => Either4::C(create),
                            Either3::C(call) => Either4::D(call),
                        })
                    })
                    .collect::<napi::Result<_>>()?;

                let bytecode = bytecode
                    .map(|b| BytecodeWrapper::new(b).into_instance(env))
                    .transpose()?;
                create.bytecode = bytecode;
                create.steps = steps;

                Ok(Either3::C(create))
            }
        }
    }

    #[napi]
    pub fn get_contract_and_function_names_for_call(
        &mut self,
        code: Uint8Array,
        calldata: Either<Uint8Array, Undefined>,
    ) -> napi::Result<ContractAndFunctionName> {
        let is_create = matches!(calldata, Either::B(()));
        let bytecode = self
            .contracts_identifier
            .get_bytecode_for_call(code.as_ref(), is_create);

        let contract = bytecode.map(|bytecode| bytecode.contract.clone());
        let contract = contract.as_ref().map(|c| c.borrow());

        let contract_name = contract.as_ref().map_or_else(
            || UNRECOGNIZED_CONTRACT_NAME.to_string(),
            |c| c.name.clone(),
        );

        if is_create {
            Ok(ContractAndFunctionName {
                contract_name,
                function_name: Either::B(()),
            })
        } else {
            match contract {
                None => Ok(ContractAndFunctionName {
                    contract_name,
                    function_name: Either::A("".to_string()),
                }),
                Some(contract) => {
                    let calldata = match calldata {
                        Either::A(calldata) => calldata,
                        Either::B(_) => {
                            unreachable!("calldata should be Some if is_create is false")
                        }
                    };

                    let selector = &calldata.get(..4).unwrap_or(&calldata[..]);

                    let func = contract.get_function_from_selector(selector);

                    let function_name = match func {
                        Some(func) => match func.r#type {
                            ContractFunctionType::Fallback => FALLBACK_FUNCTION_NAME.to_string(),
                            ContractFunctionType::Receive => RECEIVE_FUNCTION_NAME.to_string(),
                            _ => func.name.clone(),
                        },
                        None => UNRECOGNIZED_FUNCTION_NAME.to_string(),
                    };

                    Ok(ContractAndFunctionName {
                        contract_name,
                        function_name: Either::A(function_name),
                    })
                }
            }
        }
    }
}

#[napi(object)]
pub struct ContractAndFunctionName {
    pub contract_name: String,
    pub function_name: Either<String, Undefined>,
}

#[napi(catch_unwind)]
pub fn initialize_vm_trace_decoder(
    mut vm_trace_decoder: ClassInstance<VmTraceDecoder>,
    tracing_config: serde_json::Value,
) -> napi::Result<()> {
    let config = serde_json::from_value::<TracingConfig>(tracing_config).map_err(|e| {
        napi::Error::from_reason(format!("Failed to deserialize tracing config: {e:?}"))
    })?;

    let Some(build_infos) = config.build_infos else {
        return Ok(());
    };

    for build_info in &build_infos {
        let bytecodes = create_models_and_decode_bytecodes(
            build_info.solc_version.clone(),
            &build_info.input,
            &build_info.output,
        )?;

        for bytecode in bytecodes {
            if config.ignore_contracts == Some(true)
                && bytecode.contract.borrow().name.starts_with("Ignored")
            {
                continue;
            }

            vm_trace_decoder.add_bytecode_inner(Rc::new(bytecode));
        }
    }

    Ok(())
}
