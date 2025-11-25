use std::net::{Ipv6Addr, SocketAddr, TcpListener};

#[test]
fn test_sends_event() {
    let address = SocketAddr::new(Ipv6Addr::LOCALHOST.into(), 0);

    let server = TcpListener::bind(address).expect("Failed to bind server");
}
