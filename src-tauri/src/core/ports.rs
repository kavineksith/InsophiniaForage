//! Port detection and allocation. We never bind privileged ports (<1024)
//! because that would require root, which we explicitly avoid.
//!
//! On startup we check every configured port for conflicts with other
//! processes — including other instances of ourselves that may have crashed
//! and left a port held.

use crate::error::{ForageError, ForageResult};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};

/// Default port floor. We never allocate below this.
pub const PORT_MIN: u16 = 1024;
pub const PORT_MAX: u16 = 65535;

/// Check if a TCP port is free on `127.0.0.1`. We only ever bind to
/// localhost — this is a single-user dev tool, not a public server.
pub fn is_port_free(port: u16) -> bool {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    TcpListener::bind(addr).is_ok()
}

/// Validate that a port is in range and free. Returns a friendly error if
/// not.
pub fn check_port(port: u16) -> ForageResult<()> {
    if !(PORT_MIN..=PORT_MAX).contains(&port) {
        return Err(ForageError::InvalidInput(format!(
            "port {} is out of range ({}-{})",
            port, PORT_MIN, PORT_MAX
        )));
    }
    if !is_port_free(port) {
        return Err(ForageError::PortInUse { port });
    }
    Ok(())
}

/// Find a free port starting from `start`. Useful when the user picks a
/// port that turns out to be in use — we can offer the next available.
pub fn find_free_port(start: u16) -> Option<u16> {
    let mut port = start.max(PORT_MIN);
    while port <= PORT_MAX {
        if is_port_free(port) {
            return Some(port);
        }
        port = port.saturating_add(1);
    }
    None
}

/// Common suggested defaults — picked to avoid conflicts with system
/// services on Linux Mint, macOS, and Windows.
pub mod defaults {
    pub const NGINX_HTTP: u16 = 8080;
    pub const NGINX_HTTPS: u16 = 8443;
    pub const NGINX_HTTP_ALT: u16 = 8888;
    pub const NGINX_HTTPS_ALT: u16 = 8889;
    pub const MARIADB: u16 = 33060;
    pub const MARIADB_ALT: u16 = 3307;
}
