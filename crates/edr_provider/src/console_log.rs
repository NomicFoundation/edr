use edr_eth::{address, Address, Bytes};
use edr_evm::{
    inspector::Inspector,
    interpreter::{CallInputs, CallOutcome, EthInterpreter},
};

const CONSOLE_ADDRESS: Address = address!("000000000000000000636F6e736F6c652e6c6f67");

#[derive(Default)]
pub struct ConsoleLogCollector {
    encoded_messages: Vec<Bytes>,
}

impl ConsoleLogCollector {
    /// Returns the collected `console.log` messages.
    pub fn into_encoded_messages(self) -> Vec<Bytes> {
        self.encoded_messages
    }

    fn record_console_log(&mut self, encoded_message: Bytes) {
        self.encoded_messages.push(encoded_message);
    }
}

impl<ContextT> Inspector<ContextT, EthInterpreter> for ConsoleLogCollector {
    fn call(&mut self, _context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        if inputs.bytecode_address == CONSOLE_ADDRESS {
            self.record_console_log(inputs.input.clone());
        }

        None
    }
}
