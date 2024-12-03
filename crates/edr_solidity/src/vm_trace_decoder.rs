use std::rc::Rc;

use edr_eth::Bytes;
use serde::{Deserialize, Serialize};

use super::{
    message_trace::CreateMessageTrace,
    solidity_stack_trace::{
        FALLBACK_FUNCTION_NAME, RECEIVE_FUNCTION_NAME, UNRECOGNIZED_CONTRACT_NAME,
        UNRECOGNIZED_FUNCTION_NAME,
    },
};
use crate::{
    artifacts::BuildInfo,
    build_model::{Bytecode, ContractFunctionType},
    compiler::create_models_and_decode_bytecodes,
    contracts_identifier::ContractsIdentifier,
    message_trace::{MessageTrace, MessageTraceStep},
};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingConfig {
    pub build_infos: Option<Vec<BuildInfo>>,
    pub ignore_contracts: Option<bool>,
}

#[derive(Default)]
pub struct VmTraceDecoder {
    contracts_identifier: ContractsIdentifier,
}

impl VmTraceDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_bytecode(&mut self, bytecode: Bytecode) {
        self.add_bytecode_inner(Rc::new(bytecode));
    }

    pub fn add_bytecode_inner(&mut self, bytecode: Rc<Bytecode>) {
        self.contracts_identifier.add_bytecode(bytecode);
    }

    pub fn try_to_decode_message_trace(&mut self, message_trace: MessageTrace) -> MessageTrace {
        match message_trace {
            precompile @ MessageTrace::Precompile(..) => precompile,
            // NOTE: The branches below are the same with the difference of `is_create`
            MessageTrace::Call(mut call) => {
                let is_create = false;

                let bytecode = self
                    .contracts_identifier
                    .get_bytecode_for_call(call.code().as_ref(), is_create);

                let steps = call.steps().into_iter().map(|step| {
                    let trace = match step {
                        MessageTraceStep::Evm(step) => return MessageTraceStep::Evm(step),
                        MessageTraceStep::Precompile(precompile) => {
                            MessageTrace::Precompile(precompile)
                        }
                        MessageTraceStep::Create(create) => MessageTrace::Create(create),
                        MessageTraceStep::Call(call) => MessageTrace::Call(call),
                    };

                    match self.try_to_decode_message_trace(trace) {
                        MessageTrace::Precompile(precompile) => {
                            MessageTraceStep::Precompile(precompile)
                        }
                        MessageTrace::Create(create) => MessageTraceStep::Create(create),
                        MessageTrace::Call(call) => MessageTraceStep::Call(call),
                    }
                });

                call.set_bytecode(bytecode);
                call.set_steps(steps);

                MessageTrace::Call(call)
            }
            MessageTrace::Create(mut create @ CreateMessageTrace { .. }) => {
                let is_create = true;

                let bytecode = self
                    .contracts_identifier
                    .get_bytecode_for_call(create.code().as_ref(), is_create);

                let steps = create
                    .steps()
                    .into_iter()
                    .map(|step| {
                        let trace = match step {
                            MessageTraceStep::Evm(step) => return MessageTraceStep::Evm(step),
                            MessageTraceStep::Precompile(precompile) => {
                                MessageTrace::Precompile(precompile)
                            }
                            MessageTraceStep::Create(create) => MessageTrace::Create(create),
                            MessageTraceStep::Call(call) => MessageTrace::Call(call),
                        };

                        match self.try_to_decode_message_trace(trace) {
                            MessageTrace::Precompile(precompile) => {
                                MessageTraceStep::Precompile(precompile)
                            }
                            MessageTrace::Create(create) => MessageTraceStep::Create(create),
                            MessageTrace::Call(call) => MessageTraceStep::Call(call),
                        }
                    })
                    .collect::<Vec<_>>();

                create.set_bytecode(bytecode);
                create.set_steps(steps);

                MessageTrace::Create(create)
            }
        }
    }

    pub fn get_contract_and_function_names_for_call(
        &mut self,
        code: &Bytes,
        calldata: Option<&Bytes>,
    ) -> ContractAndFunctionName {
        let is_create = calldata.is_none();
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
            ContractAndFunctionName {
                contract_name,
                function_name: None,
            }
        } else {
            match contract {
                None => ContractAndFunctionName {
                    contract_name,
                    function_name: Some("".to_string()),
                },
                Some(contract) => {
                    let calldata = match calldata {
                        Some(calldata) => calldata,
                        None => {
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

                    ContractAndFunctionName {
                        contract_name,
                        function_name: Some(function_name),
                    }
                }
            }
        }
    }
}

pub struct ContractAndFunctionName {
    pub contract_name: String,
    pub function_name: Option<String>,
}

pub fn initialize_vm_trace_decoder(
    vm_trace_decoder: &mut VmTraceDecoder,
    config: TracingConfig,
) -> anyhow::Result<()> {
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
