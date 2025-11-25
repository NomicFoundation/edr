use crate::integration::common::DebuggerFixture;

fn instruction_breakpoint(pc: usize) -> edr_debugger_protocol::InstructionBreakpoint {
    edr_debugger_protocol::InstructionBreakpoint {
        condition: None,
        hit_condition: None,
        instruction_reference: pc.to_string(),
        mode: None,
        offset: None,
    }
}

fn assert_breakpoint_events(
    actual: Vec<edr_debugger_protocol::Event>,
    expected: Vec<(
        edr_debugger_protocol::BreakpointEventEvent,
        edr_debugger_protocol::InstructionBreakpoint,
    )>,
    breakpoint_id: &mut i64,
    event_id: &mut i64,
) {
    assert_eq!(actual.len(), expected.len());

    actual
        .into_iter()
        .zip(expected)
        .for_each(|(actual, expected)| {
            assert_eq!(actual.seq, *event_id);
            assert_eq!(actual.event, expected.0.to_string());

            if let Some(body) = actual.body {
                let body: edr_debugger_protocol::BreakpointEventBody =
                    serde_json::from_value(body).expect("Should deserialize");

                assert_breakpoints(vec![body.breakpoint], &vec![expected.1], breakpoint_id);
            } else {
                unreachable!("Body must be present");
            }

            *event_id += 1;
        });
}

fn assert_breakpoints(
    actual: Vec<edr_debugger_protocol::Breakpoint>,
    expected: &[edr_debugger_protocol::InstructionBreakpoint],
    breakpoint_id: &mut i64,
) {
    assert_eq!(actual.len(), expected.len());

    actual
        .into_iter()
        .zip(expected)
        .for_each(|(actual, expected)| {
            assert_eq!(actual.id, Some(*breakpoint_id));
            assert_eq!(
                actual.instruction_reference,
                Some(expected.instruction_reference.clone())
            );
            assert!(actual.message.is_none());
            assert!(actual.reason.is_none());
            assert!(actual.verified);

            *breakpoint_id += 1;
        });
}

#[test]
fn set_instruction_breakpoints_single() {
    const PC: usize = 53;

    let mut fixture = DebuggerFixture::new().expect("Failed to construct fixture");

    let breakpoint1 = instruction_breakpoint(PC);
    let expected_breakpoints = vec![breakpoint1.clone()];

    let arguments = edr_debugger_protocol::SetInstructionBreakpointsArguments {
        breakpoints: expected_breakpoints.clone(),
    };

    let response = fixture.send_request_and_wait_for_response(
        edr_debugger_protocol::SetInstructionBreakpointsRequestCommand::SetInstructionBreakpoints,
        serde_json::to_value(arguments).expect("Argument should serialize"),
    );

    let server_seq = 1;
    let mut breakpoint_id = 1;

    assert!(response.message.is_none());
    assert_eq!(response.seq, server_seq);
    assert!(response.success);

    if let Some(body) = response.body {
        let body: edr_debugger_protocol::SetInstructionBreakpointsResponseBody =
            serde_json::from_value(body).expect("Should deserialize");

        assert_breakpoints(body.breakpoints, &expected_breakpoints, &mut breakpoint_id);
    } else {
        unreachable!("Body must be present");
    }

    let mut event_seq = 1;

    let events = fixture.collect_events();
    assert_breakpoint_events(
        events,
        vec![(
            edr_debugger_protocol::BreakpointEventEvent::Breakpoint,
            breakpoint1.clone(),
        )],
        &mut breakpoint_id,
        &mut event_seq,
    );
}
