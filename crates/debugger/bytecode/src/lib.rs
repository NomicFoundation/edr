use std::{io, num::ParseIntError, str::FromStr as _};

use edr_evm_spec::{
    interpreter::{Interpreter, InterpreterTypes, Jumps as _},
    ContextTrait, Inspector,
};
use edr_primitives::IndexMap;

pub struct InstructionBreakpoint {
    /// Breakpoint ID
    id: i64,
    /// Program counter
    pc: usize,
}

impl From<&InstructionBreakpoint> for edr_debugger_protocol::Breakpoint {
    fn from(value: &InstructionBreakpoint) -> Self {
        edr_debugger_protocol::Breakpoint {
            id: Some(value.id),
            ..edr_debugger_protocol::Breakpoint::default()
        }
    }
}

pub struct BytecodeDebugger {
    /// PC -> breakpoint
    instruction_breakpoints: IndexMap<usize, InstructionBreakpoint>,
    is_paused: bool,
    next_breakpoint_id: i64,
    next_event_id: i64,
    next_response_id: i64,
    receive_request_fn: Box<dyn FnMut() -> io::Result<edr_debugger_protocol::Request>>,
    send_event_fn: Box<dyn FnMut(edr_debugger_protocol::Event) -> io::Result<()>>,
    send_response_fn: Box<dyn FnMut(edr_debugger_protocol::Response) -> io::Result<()>>,
}

impl BytecodeDebugger {
    pub fn new(
        is_paused: bool,
        receive_request_fn: Box<dyn FnMut() -> io::Result<edr_debugger_protocol::Request>>,
        send_event_fn: Box<dyn FnMut(edr_debugger_protocol::Event) -> io::Result<()>>,
        send_response_fn: Box<dyn FnMut(edr_debugger_protocol::Response) -> io::Result<()>>,
    ) -> Self {
        Self {
            is_paused,
            receive_request_fn,
            send_response_fn,
            send_event_fn,
            instruction_breakpoints: IndexMap::default(),
            next_breakpoint_id: 0,
            next_event_id: 1,
            next_response_id: 1,
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

impl Error {
    /// Retrieves the unique, backwards compatible error code for the current
    /// error.
    pub fn error_code(&self) -> i64 {
        match self {
            Error::InvalidInstructionReference(_) => 1,
            Error::MissingArguments { .. } => 2,
            Error::UnexpectedArguments { .. } => 3,
            Error::UnknownRequest { .. } => 4,
        }
    }
}

impl BytecodeDebugger {
    pub fn handle_request(
        &mut self,
        request: edr_debugger_protocol::Request,
    ) -> edr_debugger_protocol::Response {
        if edr_debugger_protocol::SetInstructionBreakpointsRequestCommand::from_str(
            &request.command,
        )
        .is_ok()
        {
            self.set_instruction_breakpoints(request)
        } else {
            self.error_response(
                &request,
                Error::UnknownRequest {
                    command: request.command.clone(),
                },
            )
        }
    }

    fn error_response(
        &mut self,
        request: &edr_debugger_protocol::Request,
        error: Error,
    ) -> edr_debugger_protocol::Response {
        let seq = self.next_response_id;
        self.next_response_id += 1;

        let id = error.error_code();
        let error = error.to_string();

        let body = edr_debugger_protocol::ErrorResponseBody {
            error: Some(edr_debugger_protocol::Message {
                id,
                // TODO: Return format and variables independently instead of using
                // thiserror to format
                format: error.clone(),
                ..edr_debugger_protocol::Message::default()
            }),
        };

        edr_debugger_protocol::Response {
            body: Some(serde_json::to_value(body).expect("Must serialize")),
            command: request.command.clone(),
            message: Some(error),
            request_seq: request.seq,
            seq,
            success: false,
            type_: edr_debugger_protocol::ResponseType::Response,
        }
    }

    fn send_event<BodyT: serde::Serialize, EventT: ToString>(
        &mut self,
        event: EventT,
        body: BodyT,
    ) -> io::Result<()> {
        let body = serde_json::to_value(body).expect("Event bodies should be serializable");

        let seq = self.next_event_id;
        self.next_event_id += 1;

        (self.send_event_fn)(edr_debugger_protocol::Event {
            body: Some(body),
            event: event.to_string(),
            seq,
            type_: edr_debugger_protocol::EventType::Event,
        })?;

        Ok(())
    }

    fn set_instruction_breakpoints(
        &mut self,
        mut request: edr_debugger_protocol::Request,
    ) -> edr_debugger_protocol::Response {
        let Some(arguments) = request.arguments.take() else {
            return self.error_response(
                &request,
                Error::MissingArguments {
                    command: request.command.clone(),
                },
            );
        };

        let Ok(arguments) = serde_json::from_value::<
            edr_debugger_protocol::SetInstructionBreakpointsArguments,
        >(arguments.clone()) else {
            return self.error_response(
                &request,
                Error::UnexpectedArguments {
                    command: request.command.clone(),
                    arguments,
                },
            );
        };

        for instruction_breakpoint in arguments.breakpoints {
            let parse_result = instruction_breakpoint.instruction_reference.clone().parse();

            let parse_result = parse_result.map(|pc| {
                let id = self.next_breakpoint_id;
                self.next_breakpoint_id += 1;

                (id, pc)
            });

            let breakpoint = edr_debugger_protocol::Breakpoint {
                column: None,
                end_column: None,
                end_line: None,
                id: parse_result.as_ref().ok().map(|(id, _pc)| *id),
                instruction_reference: Some(instruction_breakpoint.instruction_reference),
                line: None,
                message: parse_result.as_ref().err().map(ToString::to_string),
                offset: instruction_breakpoint.offset,
                reason: parse_result
                    .as_ref()
                    .err()
                    .map(|_error| edr_debugger_protocol::BreakpointReason::Failed),
                source: None,
                verified: parse_result.is_ok(),
            };

            if self
                .send_event(
                    edr_debugger_protocol::BreakpointEventEvent::Breakpoint,
                    edr_debugger_protocol::BreakpointEventBody {
                        breakpoint,
                        reason: "new".to_owned(),
                    },
                )
                .is_err()
            {
                self.on_debugger_disconnected();
            }

            let Ok((id, pc)) = parse_result else {
                continue;
            };

            let instruction_breakpoint = InstructionBreakpoint { id, pc };
            self.instruction_breakpoints
                .insert(pc, instruction_breakpoint);
        }

        let body = edr_debugger_protocol::SetInstructionBreakpointsResponseBody {
            breakpoints: self
                .instruction_breakpoints
                .values()
                .map(edr_debugger_protocol::Breakpoint::from)
                .collect(),
        };

        let body = serde_json::to_value(body).expect("Body should be serializable");

        let seq = self.next_response_id;
        self.next_response_id += 1;

        edr_debugger_protocol::Response {
            body: Some(body),
            command: request.command,
            message: None,
            request_seq: request.seq,
            seq,
            success: true,
            type_: edr_debugger_protocol::ResponseType::Response,
        }
    }

    fn terminate_debug_session(&mut self) {
        // Ignore failures as we are already terminating the debug session
        let _result = self.send_event(
            edr_debugger_protocol::TerminatedEventEvent::Terminated,
            edr_debugger_protocol::TerminatedEventBody { restart: None },
        );

        // Ensure we don't wait for continue anymore.
        self.is_paused = false;
    }

    fn on_debugger_disconnected(&mut self) {
        log::info!("Debugger disconnected");

        // If the debugger frontend disconnects, we consider the debugger frontend
        // disconnected. We can stop waiting for continue.
        self.terminate_debug_session();
    }

    /// Waits for a request to continue or terminates the debug session if the
    /// debugger frontend disconnected since we last checked.
    fn wait_for_continue(&mut self) {
        while self.is_paused {
            if let Ok(request) = (self.receive_request_fn)() {
                let response = self.handle_request(request);

                if (self.send_response_fn)(response).is_err() {
                    self.on_debugger_disconnected();
                    return;
                }
            } else {
                self.on_debugger_disconnected();
                return;
            }
        }
    }
}

impl<ContextT: ContextTrait, InterpreterT: InterpreterTypes> Inspector<ContextT, InterpreterT>
    for BytecodeDebugger
{
    #[inline]
    fn step(&mut self, interp: &mut Interpreter<InterpreterT>, _context: &mut ContextT) {
        let pc = interp.bytecode.pc();
        if let Some(instruction_breakpoint) = self.instruction_breakpoints.get(&pc) {
            if self
                .send_event(
                    edr_debugger_protocol::StoppedEventEvent::Stopped,
                    edr_debugger_protocol::StoppedEventBody {
                        hit_breakpoint_ids: vec![instruction_breakpoint.id],
                        ..edr_debugger_protocol::StoppedEventBody::default()
                    },
                )
                .is_err()
            {
                self.on_debugger_disconnected();
            };

            self.is_paused = true;
            self.wait_for_continue();
        }
    }
}
