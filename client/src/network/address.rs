//! The address of a server as the player types it: a host name or an IPv4 address, and a port
//! that defaults to the game's.

use std::fmt::{self, Display};
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

use shared::DEFAULT_PORT;

/// The address of a server: its host, as typed, and its port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerAddress {
    pub host: String,
    pub port: u16,
}

impl Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

impl ServerAddress {
    /// The same for two addresses of the same server: the host in lower case and the port.
    pub fn key(&self) -> String {
        format!("{}:{}", self.host.to_ascii_lowercase(), self.port)
    }

    /// Whether the host is an IPv4 address, which joining does not need to look up.
    #[allow(dead_code)] // The Multiplayer screen, Task 6
    pub fn is_ip(&self) -> bool {
        self.host.parse::<Ipv4Addr>().is_ok()
    }
}

/// The most characters the address field takes.
#[allow(dead_code)] // The Add server dialog, Task 5
pub const MAX_ADDRESS_LENGTH: usize = 64;
/// Shown under the address field once it has been typed into and emptied.
pub const EMPTY_ADDRESS: &str = "Enter an address";
/// Shown under the address field when the typed text is not `host` or `host:port`.
pub const INVALID_ADDRESS: &str = "Use host:port, like 10.0.0.2:8000";
/// Shown under the address field when the port is 0 or above 65535.
pub const INVALID_PORT: &str = "The port must be 1 to 65535";
/// Shown under the address field for an IPv6 address: the game socket is IPv4 only.
pub const IPV6_ADDRESS: &str = "IPv6 is not supported yet";

/// The longest host name DNS allows.
const MAX_HOST_LENGTH: usize = 253;
/// The longest label, a part between dots, of a host name.
const MAX_LABEL_LENGTH: usize = 63;

/// Whether `host` is an IPv4 address or a host name: labels of ASCII letters, digits and `-`,
/// 1 to 63 characters, neither starting nor ending with `-`, joined by dots.
fn is_valid_host(host: &str) -> bool {
    let is_label = |label: &str| {
        (1..=MAX_LABEL_LENGTH).contains(&label.len())
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    host.parse::<Ipv4Addr>().is_ok()
        || (host.len() <= MAX_HOST_LENGTH && host.split('.').all(is_label))
}

/// The address `typed` by the player, trimmed: `host` or `host:port`, the port 8000 when it is
/// left out. It is an error, the message to show, when it is empty, is not of this form, has a
/// port out of 1 to 65535, or is an IPv6 address, which the game cannot join.
pub fn parse_server_address(typed: &str) -> Result<ServerAddress, &'static str> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Err(EMPTY_ADDRESS);
    }
    if typed.starts_with('[') || typed.parse::<Ipv6Addr>().is_ok() {
        return Err(IPV6_ADDRESS);
    }
    let (host, port) = match typed.split_once(':') {
        None => (typed, DEFAULT_PORT),
        Some((host, port)) => {
            if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(INVALID_ADDRESS);
            }
            match port.parse::<u16>() {
                Ok(port) if port != 0 => (host, port),
                _ => return Err(INVALID_PORT),
            }
        }
    };
    if !is_valid_host(host) {
        return Err(INVALID_ADDRESS);
    }
    Ok(ServerAddress {
        host: host.to_string(),
        port,
    })
}

/// The first IPv4 address of `address`. Blocks on DNS: call it from a thread.
#[allow(dead_code)] // The Multiplayer screen, Task 6
pub fn resolve(address: &ServerAddress) -> io::Result<SocketAddr> {
    (address.host.as_str(), address.port)
        .to_socket_addrs()?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no IPv4 address for {}", address.host),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(host: &str, port: u16) -> ServerAddress {
        ServerAddress {
            host: host.to_string(),
            port,
        }
    }

    fn key(typed: &str) -> String {
        parse_server_address(typed).unwrap().key()
    }

    #[test]
    fn an_address_is_a_host_and_an_optional_port() {
        assert_eq!(
            parse_server_address("10.0.0.2:8000"),
            Ok(address("10.0.0.2", 8000))
        );
        let named = parse_server_address(" play.Example.com ").unwrap();
        assert_eq!(named, address("play.Example.com", DEFAULT_PORT));
        assert_eq!(named.to_string(), "play.Example.com:8000");
        assert_eq!(
            parse_server_address("localhost:25565"),
            Ok(address("localhost", 25565))
        );
    }

    #[test]
    fn bad_addresses_say_why() {
        let long_label = "x".repeat(64);
        let cases = [
            ("", EMPTY_ADDRESS),
            ("   ", EMPTY_ADDRESS),
            ("a b", INVALID_ADDRESS),
            ("host:", INVALID_ADDRESS),
            ("host:x", INVALID_ADDRESS),
            (":8000", INVALID_ADDRESS),
            ("-bad.com", INVALID_ADDRESS),
            ("a..b", INVALID_ADDRESS),
            (&long_label, INVALID_ADDRESS),
            ("a_b.com", INVALID_ADDRESS),
            ("host:0", INVALID_PORT),
            ("host:65536", INVALID_PORT),
            ("::1", IPV6_ADDRESS),
            ("[::1]:8000", IPV6_ADDRESS),
        ];
        for (typed, said) in cases {
            assert_eq!(parse_server_address(typed), Err(said), "{typed:?}");
        }
    }

    #[test]
    fn two_addresses_are_the_same_server_whatever_the_case_and_the_default_port() {
        assert_eq!(key(" LOCALHOST "), key("localhost:8000"));
        assert_ne!(key("a:8001"), key("a"));
    }

    #[test]
    fn a_host_name_resolves_to_an_ipv4_address() {
        assert_eq!(
            resolve(&address("localhost", 8000)).unwrap(),
            "127.0.0.1:8000".parse().unwrap()
        );
        assert_eq!(
            resolve(&address("10.0.0.2", 9)).unwrap(),
            "10.0.0.2:9".parse().unwrap()
        );
        assert!(resolve(&address("nowhere.invalid", 8000)).is_err());
    }
}
