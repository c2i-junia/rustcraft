//! The agent's configuration file: everything about a world's container that the hub's
//! requests do not choose.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use rustcraft_protocol::{check_hub_url, normalize_address};
use serde::Deserialize;

/// The agent's settings, read from a TOML file (`realm-agent.example.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The Unix socket the agent listens on, created with mode 0660.
    pub socket: PathBuf,
    /// The game server's image, such as `ghcr.io/c2i-junia/rustcraft/rustcraft-server:latest`.
    pub image: String,
    /// The memory of a world, as Docker's `--memory` writes it: `1536m`, `2g`.
    #[serde(default = "default_memory", deserialize_with = "memory_in_bytes")]
    pub memory: i64,
    /// The CPUs of a world.
    #[serde(default = "default_cpus")]
    pub cpus: f64,
    /// The most processes and threads of a world.
    #[serde(default = "default_pids")]
    pub pids: i64,
    /// The host's UDP port of slot 0's game; slot `n` publishes `base_port + 2n` (game) and
    /// `base_port + 2n + 1` (status).
    #[serde(default = "default_base_port")]
    pub base_port: u16,
    /// The number of slots, from 0.
    #[serde(default = "default_slots")]
    pub slots: u16,
    #[serde(default = "default_max_render_distance")]
    pub max_render_distance: u32,
    #[serde(default = "default_max_players")]
    pub max_players: u32,
    /// The hub the worlds take their members from.
    pub hub_url: String,
    /// The host players join the worlds at, without a port: `play.rustcraft.dev`.
    pub public_host: String,
    /// How long a world has to save after SIGTERM before it is killed.
    #[serde(default = "default_stop_grace_seconds")]
    pub stop_grace_seconds: u32,
    /// The Docker network of the worlds' containers.
    #[serde(default = "default_network")]
    pub network: String,
    /// How many days an archived world is kept before `purge` removes it.
    #[serde(default = "default_archive_days")]
    pub archive_days: u32,
}

fn default_memory() -> i64 {
    1536 << 20
}

fn default_cpus() -> f64 {
    1.5
}

fn default_pids() -> i64 {
    256
}

fn default_base_port() -> u16 {
    25000
}

fn default_slots() -> u16 {
    20
}

fn default_max_render_distance() -> u32 {
    12
}

fn default_max_players() -> u32 {
    10
}

fn default_stop_grace_seconds() -> u32 {
    30
}

fn default_network() -> String {
    "bridge".into()
}

fn default_archive_days() -> u32 {
    30
}

fn memory_in_bytes<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    let text = String::deserialize(deserializer)?;
    parse_memory(&text).ok_or_else(|| {
        serde::de::Error::custom(format!("{text} is not an amount of memory such as 1536m"))
    })
}

/// Bytes, or `k`, `m` or `g` (powers of 1024) after a whole number, as Docker takes them.
fn parse_memory(text: &str) -> Option<i64> {
    let text = text.trim().to_ascii_lowercase();
    let (digits, shift) = match text.strip_suffix(['k', 'm', 'g']) {
        Some(digits) => (digits, 10 * (1 + "kmg".find(text.chars().last()?)? as u32)),
        None => (text.strip_suffix('b').unwrap_or(&text), 0),
    };
    let amount: i64 = digits.parse().ok().filter(|amount| *amount > 0)?;
    amount.checked_mul(1 << shift)
}

impl Config {
    /// Reads and checks the file at `path`.
    pub fn read(path: &Path) -> anyhow::Result<Config> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        let config: Config =
            toml::from_str(&text).with_context(|| format!("{} is not valid", path.display()))?;
        config.check()?;
        Ok(config)
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.slots == 0 {
            bail!("slots must be at least 1");
        }
        if u32::from(self.base_port) + 2 * u32::from(self.slots) > u32::from(u16::MAX) + 1 {
            bail!("the ports of the last slot are beyond 65535");
        }
        if self.base_port == 0 {
            bail!("base_port must not be 0");
        }
        if !self.cpus.is_finite() || self.cpus <= 0.0 || self.pids <= 0 {
            bail!("cpus and pids must be positive");
        }
        // The server's own bounds of `--max-players`
        if !(1..=64).contains(&self.max_players) {
            bail!("max_players must be 1 to 64");
        }
        check_hub_url(&self.hub_url).map_err(anyhow::Error::msg)?;
        // A port after it would make `host:port:port`; an IPv6 address needs its brackets
        let bare_ipv6 = !self.public_host.starts_with('[') && self.public_host.contains(':');
        if bare_ipv6 || normalize_address(&format!("{}:1", self.public_host)).is_none() {
            bail!("public_host must be a host without a port, such as play.rustcraft.dev");
        }
        Ok(())
    }

    /// The host's game port of `slot`; its status port is the next one.
    pub fn game_port(&self, slot: u16) -> u16 {
        self.base_port + 2 * slot
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const EXAMPLE: &str = include_str!("../../realm-agent.example.toml");

    fn parse(text: &str) -> anyhow::Result<Config> {
        let config: Config = toml::from_str(text)?;
        config.check()?;
        Ok(config)
    }

    #[test]
    fn the_example_is_read_with_the_spec_s_values() {
        let config = parse(EXAMPLE).unwrap();
        assert_eq!(config.memory, 1536 * 1024 * 1024);
        assert_eq!(config.cpus, 1.5);
        assert_eq!(config.pids, 256);
        assert_eq!(config.base_port, 25000);
        assert_eq!(config.slots, 20);
        assert_eq!(config.max_render_distance, 12);
        assert_eq!(config.max_players, 10);
        assert_eq!(config.stop_grace_seconds, 30);
        assert_eq!(config.network, "bridge");
        assert_eq!(config.archive_days, 30);
        assert_eq!(config.game_port(19), 25038);
    }

    #[test]
    fn memory_is_read_as_docker_writes_it() {
        assert_eq!(parse_memory("1536m"), Some(1536 << 20));
        assert_eq!(parse_memory("2G"), Some(2 << 30));
        assert_eq!(parse_memory("512k"), Some(512 << 10));
        assert_eq!(parse_memory("1000"), Some(1000));
        assert_eq!(parse_memory("1000b"), Some(1000));
        for bad in ["", "m", "1.5g", "-1m", "0", "12t"] {
            assert_eq!(parse_memory(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_config_out_of_bounds_is_refused() {
        let replaced = |from: &str, to: &str| {
            assert!(EXAMPLE.contains(from), "{from}");
            parse(&EXAMPLE.replace(from, to))
        };
        assert!(replaced("slots = 20", "slots = 0").is_err());
        assert!(replaced("base_port = 25000", "base_port = 65500").is_err());
        assert!(replaced("base_port = 25000", "base_port = 65496").is_ok());
        assert!(replaced("cpus = 1.5", "cpus = 0.0").is_err());
        assert!(replaced("cpus = 1.5", "cpus = nan").is_err());
        assert!(replaced("max_players = 10", "max_players = 65").is_err());
        assert!(replaced("memory = \"1536m\"", "memory = \"lots\"").is_err());
        assert!(parse(&format!("color = \"red\"\n{EXAMPLE}")).is_err());
        assert!(replaced("https://hub.rustcraft.dev", "http://hub.rustcraft.dev").is_err());
        assert!(replaced("\"play.rustcraft.dev\"", "\"play.rustcraft.dev:25000\"").is_err());
        assert!(replaced("\"play.rustcraft.dev\"", "\"a b\"").is_err());
        assert!(replaced("\"play.rustcraft.dev\"", "\"[2001:db8::1]\"").is_ok());
        assert!(replaced("\"play.rustcraft.dev\"", "\"[2001:db8::1]:9\"").is_err());
        assert!(replaced("\"play.rustcraft.dev\"", "\"2001:db8::1\"").is_err());
    }
}
