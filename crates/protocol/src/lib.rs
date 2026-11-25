//! What the game and the hub share, with no Bevy: the identity certificates, the JSON types of
//! the hub API and the status packet codec.

pub mod api;
pub mod identity;
pub mod status;

/// Bump it whenever a network message or an enum sent in one (`BlockId`, `BlockDirection`,
/// `ItemId`, ...) changes, so that a client and a server of different builds refuse to talk
/// instead of misreading each other. The hub announces it in its well-known document.
pub const PROTOCOL_ID: u64 = 51;

/// The UDP port a server listens on when none is given, and that an address without one means.
pub const DEFAULT_PORT: u16 = 8000;

/// The hub the game talks to unless `--hub` says otherwise.
pub const DEFAULT_HUB: &str = "https://hub.rustcraft.dev";

/// The URL of a hub without its trailing `/`, the form it is compared in.
pub fn hub_base(url: &str) -> &str {
    url.trim_end_matches('/')
}

/// The host of the URL of a hub, as the messages name it: `hub.rustcraft.dev`.
pub fn hub_host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// Checks the URL of a hub for `--hub`: `https://`, or `http://` to this computer only
/// (`localhost`, `127.x.x.x`, `[::1]`), as the secrets of an account must not cross a network
/// in clear.
pub fn check_hub_url(url: &str) -> Result<String, String> {
    if let Some(rest) = url.strip_prefix("https://") {
        if !rest.is_empty() {
            return Ok(url.to_string());
        }
    } else if url.starts_with("http://") {
        let host = hub_host(url);
        let name = match host.strip_prefix('[') {
            Some(bracketed) => bracketed.split(']').next().unwrap_or_default(),
            None => host.split(':').next().unwrap_or_default(),
        };
        let loopback = name.eq_ignore_ascii_case("localhost")
            || name
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback());
        if loopback {
            return Ok(url.to_string());
        }
        return Err(format!(
            "{url}: a hub is reached over https://, http:// only on this computer"
        ));
    }
    Err(format!("{url} is not the URL of a hub (https://...)"))
}

/// The address of a server in the one form a player and its server compare: the host in lower
/// case without a trailing dot, an IPv6 address in brackets, and the port, `DEFAULT_PORT` when
/// left out: `play.example.com:8000`, `[::1]:9000`. `None` when it is not `host[:port]`.
pub fn normalize_address(address: &str) -> Option<String> {
    let address = address.trim();
    let (host, port) = if let Some(rest) = address.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        host.parse::<std::net::Ipv6Addr>().ok()?;
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':')?),
        };
        (format!("[{}]", host.to_ascii_lowercase()), port)
    } else if address.parse::<std::net::Ipv6Addr>().is_ok() {
        (format!("[{}]", address.to_ascii_lowercase()), None)
    } else {
        let (host, port) = match address.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (address, None),
        };
        let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
        let valid = !host.is_empty()
            && host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.');
        if !valid {
            return None;
        }
        (host, port)
    };
    let port = match port {
        None => DEFAULT_PORT,
        Some(port) if port.bytes().all(|byte| byte.is_ascii_digit()) => {
            port.parse::<u16>().ok().filter(|port| *port != 0)?
        }
        Some(_) => return None,
    };
    Some(format!("{host}:{port}"))
}

/// The header carrying a hosted world's token to the hub: `Authorization: Realm <token>`.
pub const REALM_AUTH_SCHEME: &str = "Realm";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_normalized_to_one_form() {
        for (typed, normal) in [
            ("Play.Example.com", "play.example.com:8000"),
            ("play.example.com.:8000", "play.example.com:8000"),
            (" 10.0.0.2:9000 ", "10.0.0.2:9000"),
            ("[::1]", "[::1]:8000"),
            ("[::1]:9000", "[::1]:9000"),
            ("::1", "[::1]:8000"),
            ("[FE80::1]:1", "[fe80::1]:1"),
        ] {
            assert_eq!(normalize_address(typed).as_deref(), Some(normal), "{typed}");
        }
        for refused in [
            "", ":8000", "a:0", "a:70000", "a:x", "a b", "[::1", "[nope]:1", "a:",
        ] {
            assert_eq!(normalize_address(refused), None, "{refused:?}");
        }
    }

    #[test]
    fn a_hub_is_reached_over_https_or_on_this_computer() {
        assert!(check_hub_url("https://hub.rustcraft.dev").is_ok());
        assert!(check_hub_url("http://127.0.0.1:8080").is_ok());
        assert!(check_hub_url("http://localhost/").is_ok());
        assert!(check_hub_url("http://[::1]:80").is_ok());
        assert!(check_hub_url("http://hub.example").is_err());
        assert!(check_hub_url("http://10.0.0.2").is_err());
        assert!(check_hub_url("hub.example").is_err());
        assert!(check_hub_url("https://").is_err());
        assert_eq!(hub_host("https://hub.example:8443/x"), "hub.example:8443");
        assert_eq!(hub_base("https://hub.example/"), "https://hub.example");
    }
}
