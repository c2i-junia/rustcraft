use bevy::prelude::*;
use bevy_renet::netcode::{
    ClientAuthentication, NetcodeClientPlugin, NetcodeClientTransport, NetcodeTransportError,
};
use bevy_renet::{renet::RenetClient, RenetClientPlugin};
use rand::Rng;
use shared::{get_shared_renet_config, GameServerConfig};

use crate::menus::solo::SelectedWorld;
use crate::network::world::update_world_from_network;
use crate::network::{update_cached_chat_state, CachedChatConversation};
use crate::player::{CurrentPlayerMarker, Player};
use crate::world::render_distance::RenderDistance;
use crate::world::time::ClientTime;
use crate::world::WorldRenderRequestUpdateEvent;
use bevy_renet::renet::DefaultChannel;
use bincode::Options;
use shared::messages::{
    AuthRegisterRequest, ChatConversation, ClientToServerMessage, PlayerId, PlayerSpawnEvent,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::{net::UdpSocket, thread, time::SystemTime};

use crate::world::ClientWorldMap;
use shared::GameFolderPaths;

#[derive(Debug, Clone, PartialEq)]
pub enum TargetServerState {
    Initial,
    Establising,
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
            name: format!("Player-{}", id),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct TargetServer {
    pub address: Option<SocketAddr>,
    pub username: Option<String>,
    pub session_token: Option<u128>,
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

fn poll_reliable_ordered_messages(
    client: &mut ResMut<RenetClient>,
    chat_state: &mut ResMut<CachedChatConversation>,
) {
    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        let message = bincode::options().deserialize::<ChatConversation>(&message);
        match message {
            Ok(data) => {
                update_cached_chat_state(chat_state, data);
            }
            Err(e) => error!("err {}", e),
        };
    }
}

fn poll_reliable_unordered_messages(
    client: &mut ResMut<RenetClient>,
    world: &mut ResMut<ClientWorldMap>,
    client_time: ResMut<ClientTime>,
    ev_render: &mut EventWriter<WorldRenderRequestUpdateEvent>,
    players: &mut Query<(&mut Transform, &Player), With<Player>>,
    current_player_entity: Query<Entity, With<CurrentPlayerMarker>>,
    render_distance: Res<RenderDistance>,
    ev_spawn: &mut EventWriter<PlayerSpawnEvent>,
) {
    update_world_from_network(
        client,
        world,
        client_time,
        ev_render,
        players,
        current_player_entity,
        render_distance,
        ev_spawn,
    );
}

pub fn poll_network_messages(
    mut client: ResMut<RenetClient>,
    mut chat_state: ResMut<CachedChatConversation>,
    client_time: ResMut<ClientTime>,
    mut world: ResMut<ClientWorldMap>,
    mut ev_render: EventWriter<WorldRenderRequestUpdateEvent>,
    mut players: Query<(&mut Transform, &Player), With<Player>>,
    current_player_entity: Query<Entity, With<CurrentPlayerMarker>>,
    render_distance: Res<RenderDistance>,
    mut ev_spawn: EventWriter<PlayerSpawnEvent>,
) {
    poll_reliable_ordered_messages(&mut client, &mut chat_state);
    poll_reliable_unordered_messages(
        &mut client,
        &mut world,
        client_time,
        &mut ev_render,
        &mut players,
        current_player_entity,
        render_distance,
        &mut ev_spawn,
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

        info!("Attempting to connect to: {}", addr);

        let authentication = ClientAuthentication::Unsecure {
            server_addr: addr,
            client_id: id,
            user_data: None,
            protocol_id: shared::PROTOCOL_ID,
        };
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

        let auth_msg = ClientToServerMessage::AuthRegisterRequest(AuthRegisterRequest {
            username: username.clone(),
        });
        let auth_msg_encoded = bincode::options().serialize(&auth_msg).unwrap();
        client.send_message(DefaultChannel::ReliableOrdered, auth_msg_encoded);
        target.state = TargetServerState::Establising;
    }

    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        let message =
            bincode::options().deserialize::<shared::messages::AuthRegisterResponse>(&message);
        if let Ok(message) = message {
            target.username = Some(message.username);
            target.session_token = Some(message.session_token);
            target.state = TargetServerState::ConnectionEstablished;
            ev_spawn.send(message.spawn_event);
            info!("Connected! {:?}", target);
        }
    }
}
