use std::{
    num::ParseIntError,
    str::FromStr as _,
    sync::mpsc::{Receiver, RecvError, Sender},
};

use edr_evm_spec::{
    interpreter::{InstructionResult, Interpreter, InterpreterTypes, Jumps as _},
    ContextTrait, Inspector,
};
use edr_primitives::IndexMap;

pub struct InstructionBreakpoint {
    /// Breakpoint ID
    id: usize,
    /// Program counter
    pc: usize,
}

impl From<&InstructionBreakpoint> for edr_debugger_protocol::Breakpoint {
    fn from(value: &InstructionBreakpoint) -> Self {
        let id = value.id.try_into().expect("Breakpoint ID fits in i64");
        edr_debugger_protocol::Breakpoint {
            id: Some(id),
            ..edr_debugger_protocol::Breakpoint::default()
        }
    }
}

pub struct BytecodeDebugger {
    // TODO: Make SPSC
    request_receiver: Receiver<edr_debugger_protocol::Request>,
    response_sender: Sender<edr_debugger_protocol::Response>,
    event_callback: Box<dyn Fn(edr_debugger_protocol::Event)>,
    /// PC -> breakpoint
    instruction_breakpoints: IndexMap<usize, InstructionBreakpoint>,
    next_breakpoint_id: usize,
    is_paused: bool,
}

impl BytecodeDebugger {
    pub fn new(
        event_callback: Box<dyn Fn(edr_debugger_protocol::Event)>,
        request_receiver: Receiver<edr_debugger_protocol::Request>,
        response_sender: Sender<edr_debugger_protocol::Response>,
        is_paused: bool,
    ) -> Self {
        Self {
            request_receiver,
            response_sender,
            event_callback,
            instruction_breakpoints: IndexMap::default(),
            next_breakpoint_id: 0,
            is_paused,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to parse instruction reference: {0}")]
    InvalidInstructionReference(ParseIntError),
    #[error("Missing arguments for command '{command}'.")]
    MissingArguments { command: String },
    #[error("Unexpected arguments for command '{command}': {arguments}")]
    UnexpectedArguments {
        command: String,
        arguments: serde_json::Value,
    },
    #[error("Received unknown request with command: '{command}'.")]
    UnknownRequest { command: String },
}

impl BytecodeDebugger {
    pub fn handle_request(
        &mut self,
        request: edr_debugger_protocol::Request,
    ) -> Result<edr_debugger_protocol::Response, Error> {
        if let Ok(command) =
            edr_debugger_protocol::SetInstructionBreakpointsRequestCommand::from_str(
                &request.command,
            )
        {
            let arguments = request
                .arguments
                .ok_or(Error::MissingArguments {
                    command: request.command,
                })
                .and_then(|arguments| {
                    serde_json::from_value::<
                        edr_debugger_protocol::SetInstructionBreakpointsArguments,
                    >(arguments)
                    .map_err(|_error| Error::UnexpectedArguments {
                        command: request.command,
                        arguments,
                    })
                })?;

            self.instruction_breakpoints = arguments
                .breakpoints
                .into_iter()
                .map(|breakpoint| {
                    let pc = breakpoint
                        .instruction_reference
                        .parse()
                        .map_err(Error::InvalidInstructionReference)?;

                    let instruction_breakpoint = InstructionBreakpoint {
                        id: self.next_breakpoint_id,
                        pc,
                    };

                    self.next_breakpoint_id += 1;

                    Ok((pc, instruction_breakpoint))
                })
                .collect::<Result<_, _>>()?;

            let body = edr_debugger_protocol::SetInstructionBreakpointsResponseBody {
                breakpoints: self
                    .instruction_breakpoints
                    .values()
                    .map(::Breakpoint::from)
                    .collect(),
            };

            Ok(response)
        };

        Err(Error::UnknownRequest {
            command: request.command,
        })
    }

    fn wait_for_continue(&mut self) -> Result<(), Error> {
        while self.is_paused {
            match self.request_receiver.recv() {
                Ok(request) => {
                    let response = self.handle_request(request)?;
                    if self.response_sender.send(response).is_err() {
                        // If the debugger frontend disconnects, we consider the debugger frontend
                        // disconnected. We can stop waiting for continue.
                        return Ok(());
                    }
                }
                // If the debugger frontend disconnects, we consider the debugger frontend
                // disconnected. We can stop waiting for continue.
                Err(RecvError) => return Ok(()),
            }
        }

        Ok(())
    }
}

impl<ContextT: ContextTrait, InterpreterT: InterpreterTypes> Inspector<ContextT, InterpreterT>
    for BytecodeDebugger
{
    #[inline]
    fn step(&mut self, interp: &mut Interpreter<InterpreterT>, _context: &mut ContextT) {
        let pc = interp.bytecode.pc();
        if let Some(instruction_breakpoint) = self.instruction_breakpoints.get(&pc) {
            (self.event_callback)(Event::Stopped {
                reason: StoppedReason::InstructionBreakpoint,
                hit_breakpoint_ids: Some(vec![instruction_breakpoint.id]),
            });

            if let Err(error) = self.wait_for_continue() {
                log::error!("{error}");

                // If an error occurs, we throw an exceptional halt to stop execution.
                interp.halt(InstructionResult::FatalExternalError);
            }
        }
    }
}
