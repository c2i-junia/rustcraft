//! What the agent asks Docker for, built from its configuration and a checked request, without
//! Docker: the names of a world's container and volumes, and the containers' specifications.

use std::collections::HashMap;

use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use rustcraft_protocol::api::RealmSettings;
use time::Date;
use uuid::Uuid;

use crate::allowlist::{DIFFICULTIES, GAME_MODES, WORLD_TYPES};
use crate::config::Config;

const PREFIX: &str = "rustcraft-realm-";
const ARCHIVED: &str = "-archived-";
/// The user of the game server's image.
const SERVER_USER: &str = "10001";
/// The game and status ports inside a world's container.
const GAME_PORT: &str = "8000/udp";
const STATUS_PORT: &str = "8001/udp";
/// Where an archive's copy mounts the volume it copies.
const COPY_SOURCE: &str = "/source";
/// The label of every container and volume of a world, whose value is the world's id.
pub const REALM_LABEL: &str = "rustcraft.realm";

/// The name of a world's container and of its volume: `rustcraft-realm-<uuid>`.
pub fn realm_name(realm: Uuid) -> String {
    format!("{PREFIX}{realm}")
}

/// The name of the volume a world is archived to on `date`:
/// `rustcraft-realm-<uuid>-archived-<yyyymmdd>`.
pub fn archived_name(realm: Uuid, date: Date) -> String {
    format!(
        "{PREFIX}{realm}{ARCHIVED}{:04}{:02}{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// The date of an archived world's volume, from its name; `None` for any other volume.
pub fn archived_on(name: &str) -> Option<Date> {
    let rest = name.strip_prefix(PREFIX)?;
    let (realm, date) = rest.split_at_checked(36)?;
    let date = date.strip_prefix(ARCHIVED)?;
    if Uuid::parse_str(realm).ok()?.to_string() != realm
        || date.len() != 8
        || !date.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let number = |range: std::ops::Range<usize>| date[range].parse::<u32>().ok();
    let month = time::Month::try_from(number(4..6)? as u8).ok()?;
    Date::from_calendar_date(number(0..4)? as i32, month, number(6..8)? as u8).ok()
}

/// Checks what a `start` request chooses: the slot, the token and the settings, which end up on
/// a container's command line and in its environment.
pub fn check_start(
    config: &Config,
    slot: u16,
    token: &str,
    settings: &RealmSettings,
) -> Result<(), String> {
    if slot >= config.slots {
        return Err(format!("slot {slot} is not below {}", config.slots));
    }
    let token_chars = |byte: u8| byte.is_ascii_alphanumeric() || b"-_=".contains(&byte);
    if !(16..=128).contains(&token.len()) || !token.bytes().all(token_chars) {
        return Err("the token is not 16 to 128 characters of base64url".into());
    }
    if settings.seed.is_some_and(|seed| seed > u64::from(u32::MAX)) {
        return Err("the seed is beyond 4294967295".into());
    }
    let allowed = |value: &Option<String>, names: &[&str], what: &str| match value {
        Some(value) if !names.contains(&value.as_str()) => {
            Err(format!("{what} must be one of {}", names.join(", ")))
        }
        _ => Ok(()),
    };
    allowed(&settings.world_type, &WORLD_TYPES, "the world type")?;
    allowed(&settings.game_mode, &GAME_MODES, "the game mode")?;
    allowed(&settings.difficulty, &DIFFICULTIES, "the difficulty")
}

/// The container of a world, once `check_start` has passed.
pub fn server_spec(
    config: &Config,
    realm: Uuid,
    slot: u16,
    token: &str,
    settings: &RealmSettings,
) -> Result<ContainerCreateBody, String> {
    check_start(config, slot, token, settings)?;
    let game_port = config.game_port(slot);
    let mut cmd: Vec<String> = [
        "--world",
        "world",
        "--port",
        "8000",
        "--auth",
        "hub",
        "--hub",
        &config.hub_url,
        "--public-address",
        &format!("{}:{game_port}", config.public_host),
        "--realm",
        &realm.to_string(),
        "--max-render-distance",
        &config.max_render_distance.to_string(),
        "--max-players",
        &config.max_players.to_string(),
    ]
    .map(String::from)
    .into();
    let flags = [
        ("--seed", settings.seed.map(|seed| seed.to_string())),
        ("--world-type", settings.world_type.clone()),
        ("--game-mode", settings.game_mode.clone()),
        ("--difficulty", settings.difficulty.clone()),
    ];
    for (flag, value) in flags {
        if let Some(value) = value {
            cmd.extend([flag.to_string(), value]);
        }
    }
    let published = |port: u16| {
        Some(
            ["0.0.0.0", "::"]
                .map(|ip| PortBinding {
                    host_ip: Some(ip.into()),
                    host_port: Some(port.to_string()),
                })
                .into(),
        )
    };
    Ok(ContainerCreateBody {
        image: Some(config.image.clone()),
        user: Some(SERVER_USER.into()),
        cmd: Some(cmd),
        env: Some(vec![format!("RUSTCRAFT_REALM_TOKEN={token}")]),
        exposed_ports: Some(vec![GAME_PORT.into(), STATUS_PORT.into()]),
        labels: Some(labels(realm)),
        stop_signal: Some("SIGTERM".into()),
        stop_timeout: Some(config.stop_grace_seconds.into()),
        host_config: Some(HostConfig {
            memory: Some(config.memory),
            memory_swap: Some(config.memory),
            nano_cpus: Some((config.cpus * 1e9) as i64),
            pids_limit: Some(config.pids),
            binds: Some(vec![format!("{}:/data", realm_name(realm))]),
            tmpfs: Some(HashMap::from([("/tmp".into(), "rw,nosuid,nodev".into())])),
            port_bindings: Some(HashMap::from([
                (GAME_PORT.into(), published(game_port)),
                (STATUS_PORT.into(), published(game_port + 1)),
            ])),
            network_mode: Some(config.network.clone()),
            ..locked_down()
        }),
        ..Default::default()
    })
}

/// The throwaway container that copies a world's volume into its archive, with the server's
/// image (whose `cp` it runs) and no network.
pub fn copy_spec(config: &Config, realm: Uuid, archive: &str) -> ContainerCreateBody {
    ContainerCreateBody {
        image: Some(config.image.clone()),
        user: Some(SERVER_USER.into()),
        entrypoint: Some(
            ["cp", "-a", &format!("{COPY_SOURCE}/."), "/data/"]
                .map(String::from)
                .into(),
        ),
        labels: Some(labels(realm)),
        host_config: Some(HostConfig {
            binds: Some(vec![
                format!("{}:{COPY_SOURCE}:ro", realm_name(realm)),
                format!("{archive}:/data"),
            ]),
            network_mode: Some("none".into()),
            ..locked_down()
        }),
        ..Default::default()
    }
}

/// The labels of a world's containers and volumes.
pub fn labels(realm: Uuid) -> HashMap<String, String> {
    HashMap::from([(REALM_LABEL.into(), realm.to_string())])
}

/// The options every container of the agent runs with.
fn locked_down() -> HostConfig {
    HostConfig {
        readonly_rootfs: Some(true),
        cap_drop: Some(vec!["ALL".into()]),
        security_opt: Some(vec!["no-new-privileges".into()]),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::tests::EXAMPLE;

    const REALM: Uuid = Uuid::from_u128(0x7a);
    const TOKEN: &str = "Ab0_-cdefghijklmnopqrstuvwxyz0123456789ABCD";

    fn config() -> Config {
        toml::from_str(EXAMPLE).unwrap()
    }

    #[test]
    fn a_world_s_names_hold_its_id_and_the_date_of_its_archive() {
        assert_eq!(
            realm_name(REALM),
            "rustcraft-realm-00000000-0000-0000-0000-00000000007a"
        );
        let date = Date::from_calendar_date(2026, time::Month::March, 4).unwrap();
        let archived = archived_name(REALM, date);
        assert_eq!(
            archived,
            "rustcraft-realm-00000000-0000-0000-0000-00000000007a-archived-20260304"
        );
        assert_eq!(archived_on(&archived), Some(date));
        for other in [
            realm_name(REALM),
            format!("{archived}1"),
            archived.replace("20260304", "20261304"),
            archived.replace("-archived-", "-archive-"),
            archived.to_uppercase(),
            "rustcraft-realm-nonsense-archived-20260304".into(),
            "db-data".into(),
        ] {
            assert_eq!(archived_on(&other), None, "{other}");
        }
    }

    #[test]
    fn a_start_out_of_the_agent_s_bounds_is_refused() {
        let config = config();
        let settings = RealmSettings::default();
        assert!(check_start(&config, 19, TOKEN, &settings).is_ok());
        assert!(check_start(&config, 20, TOKEN, &settings).is_err());
        for token in [
            "short",
            "has a space in it, see?",
            "line\nbreak-0123456789",
            &"a".repeat(129),
        ] {
            assert!(
                check_start(&config, 0, token, &settings).is_err(),
                "{token}"
            );
        }
        let with = |settings: RealmSettings| check_start(&config, 0, TOKEN, &settings);
        assert!(with(RealmSettings {
            seed: Some(u64::from(u32::MAX)),
            world_type: Some("large-biomes".into()),
            game_mode: Some("creative".into()),
            difficulty: Some("hard".into()),
        })
        .is_ok());
        assert!(with(RealmSettings {
            seed: Some(u64::from(u32::MAX) + 1),
            ..Default::default()
        })
        .is_err());
        for bad in [
            "single-biome",
            "large_biomes",
            "Default",
            "default --op x",
            "",
        ] {
            assert!(with(RealmSettings {
                world_type: Some(bad.into()),
                ..Default::default()
            })
            .is_err());
        }
        for bad in ["Survival", "spectator", "--auth"] {
            assert!(with(RealmSettings {
                game_mode: Some(bad.into()),
                ..Default::default()
            })
            .is_err());
            assert!(with(RealmSettings {
                difficulty: Some(bad.into()),
                ..Default::default()
            })
            .is_err());
        }
    }

    #[test]
    fn a_world_runs_locked_down_with_the_limits_and_ports_of_the_config() {
        let config = config();
        let settings = RealmSettings {
            seed: Some(42),
            world_type: Some("archipelago".into()),
            game_mode: None,
            difficulty: Some("easy".into()),
        };
        let spec = server_spec(&config, REALM, 3, TOKEN, &settings).unwrap();
        assert_eq!(spec.image.as_deref(), Some(config.image.as_str()));
        assert_eq!(spec.user.as_deref(), Some("10001"));
        assert_eq!(
            spec.cmd.unwrap(),
            [
                "--world",
                "world",
                "--port",
                "8000",
                "--auth",
                "hub",
                "--hub",
                "https://hub.rustcraft.dev",
                "--public-address",
                "play.rustcraft.dev:25006",
                "--realm",
                "00000000-0000-0000-0000-00000000007a",
                "--max-render-distance",
                "12",
                "--max-players",
                "10",
                "--seed",
                "42",
                "--world-type",
                "archipelago",
                "--difficulty",
                "easy",
            ]
        );
        assert_eq!(
            spec.env.unwrap(),
            [format!("RUSTCRAFT_REALM_TOKEN={TOKEN}")]
        );
        assert_eq!(spec.stop_timeout, Some(30));
        assert_eq!(spec.labels.unwrap()[REALM_LABEL], REALM.to_string());
        let host = spec.host_config.unwrap();
        assert_eq!(host.memory, Some(1536 << 20));
        assert_eq!(host.memory_swap, Some(1536 << 20));
        assert_eq!(host.nano_cpus, Some(1_500_000_000));
        assert_eq!(host.pids_limit, Some(256));
        assert_eq!(host.readonly_rootfs, Some(true));
        assert_eq!(host.cap_drop.unwrap(), ["ALL"]);
        assert_eq!(host.security_opt.unwrap(), ["no-new-privileges"]);
        assert_eq!(host.network_mode.as_deref(), Some("bridge"));
        assert_eq!(
            host.binds.unwrap(),
            ["rustcraft-realm-00000000-0000-0000-0000-00000000007a:/data"]
        );
        assert!(host.tmpfs.unwrap().contains_key("/tmp"));
        assert_eq!(host.privileged, None);
        let ports = host.port_bindings.unwrap();
        assert_eq!(ports.len(), 2);
        for (port, host_port) in [("8000/udp", "25006"), ("8001/udp", "25007")] {
            let bindings = ports[port].as_ref().unwrap();
            let ips: Vec<_> = bindings
                .iter()
                .map(|b| b.host_ip.as_deref().unwrap())
                .collect();
            assert_eq!(ips, ["0.0.0.0", "::"]);
            assert!(bindings
                .iter()
                .all(|b| b.host_port.as_deref() == Some(host_port)));
        }
    }

    #[test]
    fn no_container_is_built_for_a_refused_start() {
        let config = config();
        let settings = RealmSettings {
            game_mode: Some("creative; rm -rf /".into()),
            ..Default::default()
        };
        assert!(server_spec(&config, REALM, 0, TOKEN, &settings).is_err());
        assert!(server_spec(&config, REALM, 20, TOKEN, &Default::default()).is_err());
    }

    #[test]
    fn an_archive_is_copied_without_network_from_the_volume_read_only() {
        let config = config();
        let archive = archived_name(
            REALM,
            Date::from_calendar_date(2026, time::Month::May, 1).unwrap(),
        );
        let spec = copy_spec(&config, REALM, &archive);
        assert_eq!(
            spec.entrypoint.unwrap(),
            ["cp", "-a", "/source/.", "/data/"]
        );
        let host = spec.host_config.unwrap();
        assert_eq!(
            host.binds.unwrap(),
            [
                format!("{}:/source:ro", realm_name(REALM)),
                format!("{archive}:/data")
            ]
        );
        assert_eq!(host.network_mode.as_deref(), Some("none"));
        assert_eq!(host.readonly_rootfs, Some(true));
        assert_eq!(host.cap_drop.unwrap(), ["ALL"]);
    }
}
