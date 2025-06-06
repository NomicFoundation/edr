use alloy_primitives::{Address, Bytes, Log};
use alloy_sol_types::{SolEvent, SolInterface, SolValue};
use foundry_evm_core::{
    abi::{
        fmt::{ConsoleFmt, FormatSpec},
        patch_hh_console_selector, Console, HardhatConsole,
    },
    backend::DatabaseError,
    constants::HARDHAT_CONSOLE_ADDRESS,
    evm_context::{BlockEnvTr, ChainContextTr, HardforkTr, TransactionEnvTr},
};
use revm::{
    context::{CfgEnv, Context as EvmContext},
    interpreter::{
        CallInputs, CallOutcome, Gas, InstructionResult, Interpreter, InterpreterResult,
    },
    Database, Inspector, Journal,
};

use crate::inspectors::error_ext::ErrorExt;

/// An inspector that collects logs during execution.
///
/// The inspector collects logs from the `LOG` opcodes as well as Hardhat-style
/// logs.
#[derive(Clone, Debug, Default)]
pub struct LogCollector {
    /// The collected logs. Includes both `LOG` opcodes and Hardhat-style logs.
    pub logs: Vec<Log>,
}

impl LogCollector {
    fn hardhat_log(&mut self, mut input: Vec<u8>) -> (InstructionResult, Bytes) {
        // Patch the Hardhat-style selector (`uint` instead of `uint256`)
        patch_hh_console_selector(&mut input);

        // Decode the call
        let decoded = match HardhatConsole::HardhatConsoleCalls::abi_decode(&input, false) {
            Ok(inner) => inner,
            Err(err) => return (InstructionResult::Revert, err.abi_encode_revert()),
        };

        // Convert the decoded call to a DS `log(string)` event
        self.logs.push(convert_hh_log_to_event(decoded));

        (InstructionResult::Continue, Bytes::new())
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: Database<Error = DatabaseError>,
    >
    Inspector<
        EvmContext<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, Journal<DatabaseT>, ChainContextT>,
    > for LogCollector
{
    fn log(
        &mut self,
        _interp: &mut Interpreter,
        _context: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        log: Log,
    ) {
        self.logs.push(log);
    }

    #[inline]
    fn call(
        &mut self,
        _context: &mut EvmContext<
            BlockT,
            TxT,
            CfgEnv<HardforkT>,
            DatabaseT,
            Journal<DatabaseT>,
            ChainContextT,
        >,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        if inputs.target_address == HARDHAT_CONSOLE_ADDRESS {
            let (res, out) = self.hardhat_log(inputs.input.to_vec());
            if res != InstructionResult::Continue {
                return Some(CallOutcome {
                    result: InterpreterResult {
                        result: res,
                        output: out,
                        gas: Gas::new(inputs.gas_limit),
                    },
                    memory_offset: inputs.return_memory_offset.clone(),
                });
            }
        }

        None
    }
}

/// Converts a call to Hardhat's `console.log` to a `DSTest` `log(string)`
/// event.
fn convert_hh_log_to_event(call: HardhatConsole::HardhatConsoleCalls) -> Log {
    // Convert the parameters of the call to their string representation using
    // `ConsoleFmt`.
    let fmt = call.fmt(FormatSpec::default());
    Log::new(
        Address::default(),
        vec![Console::log::SIGNATURE_HASH],
        fmt.abi_encode().into(),
    )
    .unwrap_or_else(|| Log {
        ..Default::default()
    })
}
