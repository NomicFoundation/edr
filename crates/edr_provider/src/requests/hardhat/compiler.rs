use std::fmt::Debug;

use edr_solidity::{
    artifacts::{CompilerInput, CompilerOutput},
    compiler::create_models_and_decode_bytecodes,
};

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_add_compilation_result<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    solc_version: String,
    compiler_input: CompilerInput,
    compiler_output: CompilerOutput,
) -> Result<bool, ProviderError<LoggerErrorT>> {
    if let Err(error) = add_compilation_result_inner::<LoggerErrorT, TimerT>(
        data,
        solc_version,
        compiler_input,
        compiler_output,
    ) {
        data.logger_mut()
            .print_contract_decoding_error(&error.to_string())
            .map_err(ProviderError::Logger)?;
        Ok(false)
    } else {
        Ok(true)
    }
}

fn add_compilation_result_inner<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    solc_version: String,
    compiler_input: CompilerInput,
    compiler_output: CompilerOutput,
) -> Result<(), ProviderError<LoggerErrorT>> {
    let contracts =
        create_models_and_decode_bytecodes(solc_version, &compiler_input, &compiler_output)
            .map_err(|err| ProviderError::SolcDecoding(err.to_string()))?;

    let contract_decoder = data.contract_decoder();
    for contract in contracts {
        contract_decoder.add_contract_metadata(contract);
    }

    Ok(())
}
