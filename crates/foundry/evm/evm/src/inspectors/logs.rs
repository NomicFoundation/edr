use alloy_primitives::{Bytes, Log};
use alloy_sol_types::{SolEvent, SolInterface, SolValue};
use edr_common::fmt::ConsoleFmt;
use foundry_evm_core::{
    abi::console,
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
    fn hardhat_log(&mut self, input: Vec<u8>) -> (Option<InstructionResult>, Bytes) {
        // Decode the call
        let decoded = match console::hh::Console::ConsoleCalls::abi_decode(&input) {
            Ok(inner) => inner,
            Err(err) => return (Some(InstructionResult::Revert), err.abi_encode_revert()),
        };

        // Convert the decoded call to a DS `log(string)` event
        self.logs.push(convert_hh_log_to_event(decoded));

        (None, Bytes::new())
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
        context: &mut EvmContext<
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
            let (res, out) = self.hardhat_log(inputs.input.bytes(context).to_vec());
            if let Some(res) = res {
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

/// Converts a Hardhat `console.log` call to a DSTest `log(string)` event.
fn convert_hh_log_to_event(call: console::hh::Console::ConsoleCalls) -> Log {
    // Convert the parameters of the call to their string representation using `ConsoleFmt`.
    let msg = call.fmt(Default::default());
    new_console_log(&msg)
}

/// Creates a `console.log(string)` event.
fn new_console_log(msg: &str) -> Log {
    Log::new_unchecked(
        HARDHAT_CONSOLE_ADDRESS,
        vec![console::ds::log::SIGNATURE_HASH],
        msg.abi_encode().into(),
    )
}
