use alloy_primitives::Log;
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
use revm::context::ContextTr;
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
    #[cold]
    fn do_hardhat_log<CTX>(&mut self, context: &mut CTX, inputs: &CallInputs) -> Option<CallOutcome>
    where
        CTX: ContextTr,
    {
        if let Err(err) = self.hardhat_log(&inputs.input.bytes(context)) {
            let result = InstructionResult::Revert;
            let output = err.abi_encode_revert();
            return Some(CallOutcome {
                result: InterpreterResult { result, output, gas: Gas::new(inputs.gas_limit) },
                memory_offset: inputs.return_memory_offset.clone(),
            });
        }
        None
    }

    fn hardhat_log(&mut self, data: &[u8]) -> alloy_sol_types::Result<()> {
        let decoded = console::hh::ConsoleCalls::abi_decode(data)?;
        self.logs.push(hh_to_ds(&decoded));
        Ok(())
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
            return self.do_hardhat_log(context, inputs);
        }
        None
    }
}

/// Converts a Hardhat `console.log` call to a DSTest `log(string)` event.
fn hh_to_ds(call: &console::hh::ConsoleCalls) -> Log {
    // Convert the parameters of the call to their string representation using `ConsoleFmt`.
    let msg = call.fmt(Default::default());
    new_console_log(&msg)
}

/// Creates a `console.log(string)` event.
pub(crate) fn new_console_log(msg: &str) -> Log {
    Log::new_unchecked(
        HARDHAT_CONSOLE_ADDRESS,
        vec![console::ds::log::SIGNATURE_HASH],
        msg.abi_encode().into(),
    )
}
