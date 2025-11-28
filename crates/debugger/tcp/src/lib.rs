use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
};

use edr_debugger_bytecode::BytecodeDebugger;

/// Creates a TCP-based bytecode debugger that connects to the specified server
/// address.
pub fn create_tcp_debugger(
    server_address: SocketAddr,
    is_paused: bool,
) -> io::Result<BytecodeDebugger> {
    let mut event_stream = TcpStream::connect(server_address)?;
    let mut request_stream = event_stream.try_clone()?;
    let mut response_stream = event_stream.try_clone()?;

    let receive_request_fn = Box::new(move || {
        let mut buffer = String::new();
        request_stream.read_to_string(&mut buffer)?;

        println!("Received request: {}", buffer);

        let request: edr_debugger_protocol::Request = serde_json::from_str(&buffer)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        Ok(request)
    });

    let send_event_fn = Box::new(move |event: edr_debugger_protocol::Event| {
        let json = serde_json::to_string(&event).expect("Failed to serialize event");
        event_stream.write_all(json.as_bytes())
    });

    let send_response_fn = Box::new(move |response: edr_debugger_protocol::Response| {
        let json = serde_json::to_string(&response).expect("Failed to serialize response");
        response_stream.write_all(json.as_bytes())
    });

    Ok(BytecodeDebugger::new(
        is_paused,
        receive_request_fn,
        send_event_fn,
        send_response_fn,
    ))
}
