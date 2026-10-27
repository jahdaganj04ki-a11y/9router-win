// Port-Discovery: 127.0.0.1:port probieren; erfolgreicher Connect = belegt.

use std::net::TcpStream;
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 20128;
pub const PORT_RANGE_END: u16 = 20137;

pub fn port_in_use(port: u16) -> bool {
    let addr = format!("127.0.0.1:{}", port);
    TcpStream::connect_timeout(&addr.parse::<std::net::SocketAddr>().expect("gültige Adresse"), Duration::from_millis(100))
        .is_ok()
}

pub fn find_free_port(start: u16, max_attempts: u16) -> Option<u16> {
    for i in 0..max_attempts {
        let port = start.saturating_add(i);
        if port > PORT_RANGE_END {
            break;
        }
        if !port_in_use(port) {
            return Some(port);
        }
    }
    None
}

pub fn wait_for_port(port: u16, timeout: Duration, interval: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if port_in_use(port) {
            return true;
        }
        std::thread::sleep(interval);
    }
    port_in_use(port)
}
