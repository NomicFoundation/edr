use crate::integration::common::{ResponseAndEvents, TcpDebuggerFixture};

fn instruction_breakpoint(pc: usize) -> edr_debugger_protocol::InstructionBreakpoint {
    edr_debugger_protocol::InstructionBreakpoint {
        condition: None,
        hit_condition: None,
        instruction_reference: pc.to_string(),
        mode: None,
        offset: None,
    }
}

#[test]
fn test_sends_event() {
    const PC: usize = 53;

    let mut fixture = TcpDebuggerFixture::new().expect("Failed to create fixture");

    let breakpoint1 = instruction_breakpoint(PC);

    let ResponseAndEvents { events, .. } = fixture.send_request_and_wait_for_protocol_messages(
        edr_debugger_protocol::SetInstructionBreakpointsRequestCommand::SetInstructionBreakpoints,
        edr_debugger_protocol::SetInstructionBreakpointsArguments {
            breakpoints: vec![breakpoint1.clone()],
        },
    );

    assert_eq!(events.len(), 1, "Expected exactly one event");
}
