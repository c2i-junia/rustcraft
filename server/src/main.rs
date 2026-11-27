use std::net::{IpAddr, Ipv4Addr};
use std::process::ExitCode;

use bevy::log::{error, tracing_subscriber::EnvFilter, warn};
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Parser;
use rustcraft_protocol::{check_hub_url, normalize_address, DEFAULT_HUB};
use server::acquire_socket_by_port;
use server::hub::{watch_realm, RealmLink, REALM_EVERY};
use shared::players::game_mode::GameMode;
use shared::status::status_port;
use shared::world::hostiles::Difficulty;
use shared::world::{WorldGenSettings, WorldPreset};
use shared::{
    get_game_folder_paths, GameServerConfig, NewWorldSettings, ServerAuth,
    DEFAULT_MAX_RENDER_DISTANCE, DEFAULT_PORT, MAX_PLAYERS,
};
use uuid::Uuid;

/// The environment variable that holds the token of a hosted world: not a flag, which `ps` shows.
const REALM_TOKEN_VARIABLE: &str = "RUSTCRAFT_REALM_TOKEN";

/// The exit code of a process interrupted twice: 128 + SIGINT.
const INTERRUPTED_EXIT_CODE: i32 = 130;

/// The world types a flag takes: the names of the game, in kebab-case.
fn world_types() -> PossibleValuesParser {
    // Leaked once, as the parser keeps its names for the life of the process
    PossibleValuesParser::new(
        WorldPreset::NAMES.map(|name| &*Box::leak(name.replace('_', "-").into_boxed_str())),
    )
}

fn game_modes() -> impl clap::builder::TypedValueParser<Value = GameMode> {
    PossibleValuesParser::new(GameMode::ALL.map(GameMode::name))
        .map(|name| GameMode::from_name(&name).expect("a name of the game"))
}

fn difficulties() -> impl clap::builder::TypedValueParser<Value = Difficulty> {
    PossibleValuesParser::new(Difficulty::ALL.map(Difficulty::name))
        .map(|name| Difficulty::from_name(&name).expect("a name of the game"))
}

/// How the server knows who its players are.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum AuthFlag {
    /// A player is who it says it is
    Offline,
    /// A player needs an account of the hub of `--hub`
    Hub,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    port: u16,

    #[arg(short, long, default_value = "default")]
    world: String,

    #[arg(short, long)]
    game_folder_path: Option<String>,

    /// The data folder, whose `content/rustcraft/*.ron` define the properties of the blocks
    /// and items, read again once saved
    #[arg(long)]
    assets_folder_path: Option<String>,

    /// The largest render distance a player may ask for, in chunks
    #[arg(long, default_value_t = DEFAULT_MAX_RENDER_DISTANCE)]
    max_render_distance: u32,

    /// Makes this player an operator before the server starts, in `ops.ron` of the game
    /// folder: how a server gets its first operator. May be given several times
    #[arg(long = "op", value_name = "NAME")]
    ops: Vec<String>,

    /// The most players connected at once
    #[arg(
        long,
        default_value_t = MAX_PLAYERS as u32,
        value_parser = clap::value_parser!(u32).range(1..=MAX_PLAYERS as i64),
    )]
    max_players: u32,

    /// The seed of the world, when it is created: random by default
    #[arg(long)]
    seed: Option<u32>,

    /// The type of the world, when it is created
    #[arg(long, value_parser = world_types())]
    world_type: Option<String>,

    /// The biome of a `single-biome` world, by its key, such as `plains`
    #[arg(long)]
    biome: Option<String>,

    /// The game mode of the players of the world, when it is created: survival by default
    #[arg(long, value_parser = game_modes())]
    game_mode: Option<GameMode>,

    /// The difficulty of the world, when it is created: normal by default
    #[arg(long, value_parser = difficulties())]
    difficulty: Option<Difficulty>,

    /// Who may join: anyone by the name it gives, or the accounts of the hub only
    #[arg(long, value_enum, default_value_t = AuthFlag::Offline)]
    auth: AuthFlag,

    /// The hub whose accounts join in `--auth hub`: https://, or http:// on this computer only
    #[arg(long, default_value = DEFAULT_HUB, value_parser = check_hub_url)]
    hub: String,

    /// Runs the hosted world of this id of the hub, whose members are the hub's: needs
    /// `--auth hub`, and the realm token in the environment variable RUSTCRAFT_REALM_TOKEN
    #[arg(long, value_name = "UUID", conflicts_with = "ops")]
    realm: Option<Uuid>,

    /// An address players join this server at, as they type it (`host[:port]`, port 8000 by
    /// default): a proof of an account made for another is refused. Needed with `--auth hub`;
    /// may be given several times
    #[arg(long = "public-address", value_name = "HOST[:PORT]", value_parser = public_address)]
    public_addresses: Vec<String>,
}

fn public_address(typed: &str) -> Result<String, String> {
    normalize_address(typed).ok_or_else(|| format!("{typed} is not host[:port]"))
}

/// Whether `address` (`host:port`, normalized) is one that other machines may have too:
/// `localhost`, or a loopback, private, link-local or unspecified IP address.
fn is_local_address(address: &str) -> bool {
    let host = address.rsplit_once(':').map_or(address, |(host, _)| host);
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified()
        }
        Ok(IpAddr::V6(ip)) => {
            ip.is_loopback()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_unspecified()
        }
        Err(_) => false,
    }
}

/// The local addresses of `addresses` when a public one is among them: a proof made for a
/// server at the same local address elsewhere could then be relayed to this public server.
fn local_beside_public(addresses: &[String]) -> Vec<&str> {
    if addresses.iter().all(|address| is_local_address(address)) {
        return Vec::new();
    }
    addresses
        .iter()
        .map(String::as_str)
        .filter(|address| is_local_address(address))
        .collect()
}

impl Args {
    /// What the options say of a new world, or why they contradict each other.
    fn new_world(&self) -> Result<NewWorldSettings, String> {
        let preset = match (self.world_type.as_deref(), &self.biome) {
            (Some("single-biome"), Some(biome)) => {
                WorldPreset::from_name(&format!("single_biome:{biome}"))
                    .ok_or("--biome is empty")?
                    .into()
            }
            (Some("single-biome"), None) => {
                return Err("--world-type single-biome needs --biome".into())
            }
            (Some(_), Some(_)) | (None, Some(_)) => {
                return Err("--biome goes with --world-type single-biome".into())
            }
            (Some(name), None) => WorldPreset::from_name(&name.replace('-', "_")),
            (None, None) => None,
        };
        Ok(NewWorldSettings {
            seed: self.seed,
            generation: preset.map(|preset| WorldGenSettings {
                preset,
                ..Default::default()
            }),
            game_mode: self.game_mode,
            difficulty: self.difficulty,
        })
    }
}

fn main() -> ExitCode {
    let args = Args::parse();
    bevy::log::tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    // SIGTERM (`docker stop`) and Ctrl-C save the world and exit; a second signal exits at once
    if let Err(err) = ctrlc::set_handler(|| {
        if server::request_shutdown() {
            std::process::exit(INTERRUPTED_EXIT_CODE);
        }
    }) {
        warn!("Could not catch the stop signals, the world will not be saved on them: {err}");
    }
    let new_world = match args.new_world() {
        Ok(new_world) => new_world,
        Err(err) => {
            error!("{err}");
            return ExitCode::FAILURE;
        }
    };
    let ip = Ipv4Addr::UNSPECIFIED.into();
    let socket = match acquire_socket_by_port(ip, args.port) {
        Ok(socket) => socket,
        Err(err) => {
            error!("Could not listen on UDP port {}: {err}", args.port);
            return ExitCode::FAILURE;
        }
    };
    // The port actually bound, which `--port 0` leaves to the system
    let game_port = socket.local_addr().map_or(args.port, |addr| addr.port());
    // Without a status port the server still runs: the server list shows it offline
    let status = match status_port(game_port) {
        Some(port) => match acquire_socket_by_port(ip, port) {
            Ok(status) => Some(status),
            Err(err) => {
                warn!("Could not listen on UDP port {port} for the status: {err}; the server list will show this server offline");
                None
            }
        },
        None => {
            warn!("UDP port {game_port} has no port next to it for the status; the server list will show this server offline");
            None
        }
    };

    let game_folder_paths = get_game_folder_paths(args.game_folder_path, args.assets_folder_path);
    let auth = match args.auth {
        AuthFlag::Offline => ServerAuth::Offline,
        AuthFlag::Hub if args.public_addresses.is_empty() => {
            error!("--auth hub needs --public-address: the address players join this server at");
            return ExitCode::FAILURE;
        }
        AuthFlag::Hub => {
            let local = local_beside_public(&args.public_addresses);
            if !local.is_empty() {
                warn!(
                    "--public-address lists local addresses beside public ones: {}. A server at the same address on another network could pass this one the proofs of its players; give them only for a server reached on this network alone",
                    local.join(", ")
                );
            }
            match server::hub::hub_trust(
                &args.hub,
                args.public_addresses.clone(),
                &game_folder_paths.game_folder_path,
            ) {
                Ok(trust) => {
                    server::hub::watch_revocations(&trust);
                    ServerAuth::Hub(trust)
                }
                Err(err) => {
                    error!("{err}");
                    return ExitCode::FAILURE;
                }
            }
        }
    };
    // A hosted world reads its members now: it does not start without them
    let realm = match (args.realm, &auth) {
        (None, _) => None,
        (Some(id), ServerAuth::Hub(trust)) => {
            let Ok(token) = std::env::var(REALM_TOKEN_VARIABLE) else {
                error!("--realm needs the token of the world in {REALM_TOKEN_VARIABLE}");
                return ExitCode::FAILURE;
            };
            match watch_realm(RealmLink::new(&trust.url, id, token), REALM_EVERY) {
                Ok(watch) => Some(watch),
                Err(err) => {
                    error!("{err}");
                    return ExitCode::FAILURE;
                }
            }
        }
        (Some(_), ServerAuth::Offline) => {
            error!("--realm needs --auth hub");
            return ExitCode::FAILURE;
        }
    };
    let config = GameServerConfig {
        world_name: args.world,
        is_solo: false,
        saves: None,
        max_render_distance: args.max_render_distance,
        max_players: args.max_players as usize,
        new_world,
        auth,
        content: shared::ContentSource::WatchedFiles,
    };
    if !args.ops.is_empty() {
        let folder = &game_folder_paths.game_folder_path;
        // In hub mode an operator is an account: the hub tells which
        let players = match &config.auth {
            ServerAuth::Offline => Ok(args.ops.iter().map(server::Entry::named).collect()),
            ServerAuth::Hub(trust) => server::hub::resolve_names(&trust.url, &args.ops),
        };
        let added =
            players.and_then(|players| server::Permissions::add_operators(folder, &players));
        if let Err(err) = added {
            error!("{err}");
            return ExitCode::FAILURE;
        }
    }
    let result = match (realm, status) {
        (Some(realm), status) => {
            server::init_hosted(socket, status, config, game_folder_paths, realm)
        }
        (None, Some(status)) => server::init_with_status(socket, status, config, game_folder_paths),
        (None, None) => server::init(socket, config, game_folder_paths),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_world(args: &[&str]) -> Result<NewWorldSettings, String> {
        Args::try_parse_from(["server"].into_iter().chain(args.iter().copied()))
            .map_err(|err| err.to_string())?
            .new_world()
    }

    #[test]
    fn every_world_type_of_the_game_is_taken_in_kebab_case() {
        for name in WorldPreset::NAMES {
            let kebab = name.replace('_', "-");
            let mut args = vec!["--world-type", &kebab];
            if name == "single_biome" {
                args.extend(["--biome", "plains"]);
            }
            let generation = new_world(&args).unwrap().generation;
            assert_eq!(
                generation.map(|g| g.preset.name()).as_deref().map(|n| n
                    .split(':')
                    .next()
                    .unwrap()
                    .to_string()),
                Some(name.to_string())
            );
        }
    }

    #[test]
    fn every_game_mode_and_difficulty_of_the_game_is_taken() {
        for mode in GameMode::ALL {
            let settings = new_world(&["--game-mode", mode.name()]).unwrap();
            assert_eq!(settings.game_mode, Some(mode));
        }
        for difficulty in Difficulty::ALL {
            let settings = new_world(&["--difficulty", difficulty.name()]).unwrap();
            assert_eq!(settings.difficulty, Some(difficulty));
        }
    }

    /// The hub's copy of the world settings a hosted world may have.
    mod agent_allowlist {
        include!("../../hub/agent-protocol/src/allowlist.rs");
    }

    #[test]
    fn the_realm_agent_allows_the_world_settings_of_the_game() {
        let world_types: Vec<String> = WorldPreset::NAMES
            .into_iter()
            .filter(|name| *name != "single_biome")
            .map(|name| name.replace('_', "-"))
            .collect();
        assert_eq!(agent_allowlist::WORLD_TYPES.as_slice(), world_types);
        assert_eq!(
            agent_allowlist::GAME_MODES,
            GameMode::ALL.map(GameMode::name)
        );
        assert_eq!(
            agent_allowlist::DIFFICULTIES,
            Difficulty::ALL.map(Difficulty::name)
        );
        for world_type in agent_allowlist::WORLD_TYPES {
            assert!(
                new_world(&["--world-type", world_type]).is_ok(),
                "{world_type}"
            );
        }
    }

    #[test]
    fn a_biome_goes_with_the_single_biome_type_only() {
        assert!(new_world(&["--world-type", "single-biome"]).is_err());
        assert!(new_world(&["--biome", "plains"]).is_err());
        assert!(new_world(&["--world-type", "archipelago", "--biome", "plains"]).is_err());
        assert!(new_world(&["--world-type", "nonsense"]).is_err());
        assert!(new_world(&["--max-players", "65"]).is_err());
    }

    #[test]
    fn the_hub_and_the_public_addresses_are_checked() {
        let parse = |args: &[&str]| {
            Args::try_parse_from(["server"].into_iter().chain(args.iter().copied()))
        };
        assert!(parse(&["--hub", "http://hub.example"]).is_err());
        assert!(parse(&["--hub", "http://127.0.0.1:8080"]).is_ok());
        let args = parse(&[
            "--public-address",
            "Play.Example.com",
            "--public-address",
            "[::1]:9",
        ])
        .unwrap();
        assert_eq!(args.public_addresses, ["play.example.com:8000", "[::1]:9"]);
        assert!(parse(&["--public-address", "a b"]).is_err());
    }

    #[test]
    fn loopback_private_and_link_local_addresses_are_local() {
        for local in [
            "localhost:8000",
            "a.localhost:8000",
            "127.0.0.1:8000",
            "10.1.2.3:8000",
            "172.16.0.1:8000",
            "192.168.1.10:8000",
            "169.254.0.1:8000",
            "0.0.0.0:8000",
            "[::1]:8000",
            "[fd30:80:81::1]:8000",
            "[fe80::1]:8000",
        ] {
            assert!(is_local_address(local), "{local}");
        }
        for public in [
            "play.example.com:8000",
            "203.0.113.7:8000",
            "172.32.0.1:8000",
            "[2001:db8::1]:8000",
        ] {
            assert!(!is_local_address(public), "{public}");
        }
    }

    #[test]
    fn local_addresses_are_told_only_beside_public_ones() {
        let addresses = |list: &[&str]| list.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        assert!(local_beside_public(&addresses(&["play.example.com:8000"])).is_empty());
        assert!(local_beside_public(&addresses(&["127.0.0.1:8000", "[::1]:8000"])).is_empty());
        assert_eq!(
            local_beside_public(&addresses(&[
                "play.example.com:8000",
                "192.168.1.10:8000",
                "localhost:8000"
            ])),
            ["192.168.1.10:8000", "localhost:8000"]
        );
    }

    #[test]
    fn a_hosted_world_takes_an_id_and_no_operator_option() {
        let parse = |args: &[&str]| {
            Args::try_parse_from(["server"].into_iter().chain(args.iter().copied()))
        };
        let id = "00000000-0000-0000-0000-00000000007a";
        assert_eq!(
            parse(&["--realm", id]).unwrap().realm,
            Some(Uuid::from_u128(0x7a))
        );
        assert!(parse(&["--realm", "nonsense"]).is_err());
        assert!(parse(&["--realm", id, "--op", "Alex"]).is_err());
    }
}
