use edr_chain_spec_evm::{
    interpreter::{CallInputs, CallOutcome, EthInterpreter},
    ContextTrait, Inspector,
};
use edr_primitives::{address, Address, Bytes};

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

    /// Takes the collected `console.log` messages, leaving an empty collection.
    pub fn take_encoded_messages(&mut self) -> Vec<Bytes> {
        std::mem::take(&mut self.encoded_messages)
    }

    fn record_console_log(&mut self, encoded_message: Bytes) {
        self.encoded_messages.push(encoded_message);
    }
}

impl<ContextT: ContextTrait> Inspector<ContextT, EthInterpreter> for ConsoleLogCollector {
    fn call(&mut self, context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        if inputs.bytecode_address == CONSOLE_ADDRESS {
            self.record_console_log(inputs.input.bytes(context));
        }

        None
    }
}
