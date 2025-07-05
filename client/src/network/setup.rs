use bevy::prelude::*;
use bevy_renet::netcode::{
    ClientAuthentication, NetcodeClientPlugin, NetcodeClientTransport, NetcodeTransportError,
};
use bevy_renet::{renet::RenetClient, RenetClientPlugin};
use rand::Rng;
use shared::messages::mob::MobUpdateEvent;
use shared::{get_shared_renet_config, GameServerConfig, STC_AUTH_CHANNEL};

use crate::menus::solo::SelectedWorld;
use crate::network::world::update_world_from_network;
use crate::network::CachedChatConversation;
use crate::world::time::ClientTime;
use crate::world::WorldRenderRequestUpdateEvent;
use crate::PlayerNameSupplied;
use shared::messages::{
    AuthRegisterRequest, ItemStackUpdateEvent, PlayerId, PlayerSpawnEvent, PlayerUpdateEvent,
    ServerToClientMessage,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::{net::UdpSocket, thread, time::SystemTime};

use crate::world::ClientWorldMap;
use shared::GameFolderPaths;

use super::SendGameMessageExtension;

#[derive(Debug, Clone, PartialEq)]
pub enum TargetServerState {
    Initial,
    Establishing,
    ConnectionEstablished,
    FullyReady, // player has spawned
}

#[derive(Resource, Clone)]
pub struct CurrentPlayerProfile {
    pub id: PlayerId,
    pub name: String,
}

impl CurrentPlayerProfile {
    pub(crate) fn new() -> Self {
        let mut rng = rand::thread_rng();
        let id: u64 = rng.gen();
        Self {
            id,
            name: format!("Player-{id}"),
        }
    }
}

fn hash_string_to_u64(input: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}

impl FromWorld for CurrentPlayerProfile {
    fn from_world(world: &mut World) -> Self {
        let player_name = world.get_resource::<PlayerNameSupplied>();
        match player_name {
            Some(player_name) => Self {
                id: hash_string_to_u64(&player_name.name),
                name: player_name.name.clone(),
            },
            None => CurrentPlayerProfile::new(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct TargetServer {
    pub address: Option<SocketAddr>,
    pub username: Option<String>,
    pub session_token: Option<u64>,
    pub state: TargetServerState,
}

pub fn add_base_netcode(app: &mut App) {
    app.add_plugins(RenetClientPlugin);

    let client = RenetClient::new(get_shared_renet_config());
    app.insert_resource(client);

    // Setup the transport layer
    app.add_plugins(NetcodeClientPlugin);

    // TODO: change username
    app.insert_resource(TargetServer {
        address: None,
        username: None,
        session_token: None,
        state: TargetServerState::Initial,
    });
}

pub fn launch_local_server_system(
    mut target: ResMut<TargetServer>,
    selected_world: Res<SelectedWorld>,
    paths: Res<GameFolderPaths>,
) {
    if target.address.is_some() {
        debug!("Skipping launch local server");
        return;
    }

    if let Some(world_name) = &selected_world.name {
        info!("Launching local server with world: {}", world_name);

        let socket =
            server::acquire_local_ephemeral_udp_socket(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        let addr = socket.local_addr().unwrap();
        debug!("Obtained UDP socket: {}", addr);

        let world_name_clone = world_name.clone();
        let game_folder_path = paths.clone().game_folder_path;
        //
        thread::spawn(move || {
            server::init(
                socket,
                GameServerConfig {
                    world_name: world_name_clone,
                    is_solo: true,
                },
                game_folder_path,
            );
        });

        target.address = Some(addr);
    } else {
        error!("Error: No world selected. Unable to launch the server.");
    }
}

pub fn poll_network_messages(
    mut client: ResMut<RenetClient>,
    // mut chat_state: ResMut<CachedChatConversation>,
    // client_time: ResMut<ClientTime>,
    mut world: ResMut<ClientWorldMap>,
    mut ev_render: EventWriter<WorldRenderRequestUpdateEvent>,
    mut ev_player_spawn: EventWriter<PlayerSpawnEvent>,
    mut ev_mob_update: EventWriter<MobUpdateEvent>,
    mut ev_item_stacks_update: EventWriter<ItemStackUpdateEvent>,
    mut ev_player_update: EventWriter<PlayerUpdateEvent>,
) {
    // poll_reliable_ordered_messages(&mut client, &mut chat_state);
    update_world_from_network(
        &mut client,
        &mut world,
        &mut ev_render,
        &mut ev_player_spawn,
        &mut ev_mob_update,
        &mut ev_item_stacks_update,
        &mut ev_player_update,
    );
}

pub fn init_server_connection(
    mut commands: Commands,
    target: Res<TargetServer>,
    current_player_id: Res<CurrentPlayerProfile>,
) {
    let addr = target.address.unwrap();
    let id = current_player_id.into_inner().id;
    commands.queue(move |world: &mut World| {
        world.remove_resource::<RenetClient>();
        world.remove_resource::<NetcodeClientTransport>();
        world.remove_resource::<CachedChatConversation>();

        let client = RenetClient::new(get_shared_renet_config());
        world.insert_resource(client);

        let authentication = ClientAuthentication::Unsecure {
            server_addr: addr,
            client_id: id,
            user_data: None,
            protocol_id: shared::PROTOCOL_ID,
        };

        info!(
            "Attempting to connect to: {} with data {:?}",
            addr, authentication
        );

        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

        world.insert_resource(transport);

        world.insert_resource(CachedChatConversation { ..default() });

        info!("Network subsystem initialized");
    })
}

pub fn network_failure_handler(mut renet_error: EventReader<NetcodeTransportError>) {
    for e in renet_error.read() {
        error!("network error: {}", e);
    }
}

pub fn establish_authenticated_connection_to_server(
    mut client: ResMut<RenetClient>,
    mut target: ResMut<TargetServer>,
    current_profile: Res<CurrentPlayerProfile>,
    mut ev_spawn: EventWriter<PlayerSpawnEvent>,
    mut client_time: ResMut<ClientTime>,
) {
    if target.session_token.is_some() {
        info!(
            "Successfully acquired a session token as {}",
            &target.username.clone().unwrap()
        );
        return;
    }

    if target.state == TargetServerState::Initial {
        if target.username.is_none() {
            target.username = Some(current_profile.into_inner().name.clone());
        }

        let username = target.username.as_ref().unwrap();

        let auth_msg = AuthRegisterRequest {
            username: username.clone(),
        };
        info!("Sending auth request: {:?}", auth_msg);
        client.send_game_message(auth_msg.into());
        target.state = TargetServerState::Establishing;
    }

    while let Some(Ok(message)) = client.receive_game_message_by_channel(STC_AUTH_CHANNEL) {
        match message {
            ServerToClientMessage::AuthRegisterResponse(message) => {
                target.username = Some(message.username);
                target.session_token = Some(message.session_token);
                target.state = TargetServerState::ConnectionEstablished;
                client_time.0 = message.tick;
                // TODO: handle clock sync using the timestamp_ms field
                // it will become very important if the lantency is high
                for player in message.players {
                    ev_spawn.send(player);
                }
                info!("Connected! {:?}", target);
            }
            _ => {
                panic!("Unexpected message: {message:?}");
            }
        }
    }
}
