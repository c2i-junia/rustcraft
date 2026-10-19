pub mod dialogs;

use super::list_screen::*;
use super::servers::{read_servers, remove_server, ServerItem};
use super::MenuState;
use crate::constants::SERVER_LIST_SAVE_NAME;
use crate::network::{TargetServer, TargetServerState};
use crate::state::GameState;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use dialogs::{open_add_server_dialog, ServersChanged};
use shared::GameFolderPaths;
use std::net::SocketAddr;

/// Parses the address of a server as typed by the user: an IP address and a port, such as
/// `127.0.0.1:8000`. Host names are not resolved.
fn parse_server_address(text: &str) -> Option<SocketAddr> {
    text.parse().ok()
}

#[derive(Component, Default)]
pub struct ServerList {
    /// The servers in the order they are shown, which is the order they are saved in.
    pub servers: Vec<(Entity, ServerItem)>,
}

impl ServerList {
    /// The listed servers, in order.
    fn items(&self) -> Vec<ServerItem> {
        self.servers.iter().map(|(_, srv)| srv.clone()).collect()
    }
}

#[derive(Component)]
pub enum MultiplayerButtonAction {
    Connect(Entity),
    Delete(Entity),
}

/// The Multiplayer screen and its dialogs.
pub fn multi_plugin(app: &mut App) {
    dialogs::plugin(app);
    app.add_systems(OnEnter(MenuState::Multi), multiplayer_menu_setup)
        .add_systems(
            Update,
            multiplayer_action.run_if(in_state(MenuState::Multi)),
        )
        .add_observer(list_changed_servers);
}

/// Spawns the screen, then lists the servers.
pub fn multiplayer_menu_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let screen = spawn_list_screen(
        &mut commands,
        &asset_server,
        &ListScreen {
            menu: MenuState::Multi,
            title: "Server list",
            add_label: "Add server",
        },
    );
    commands.entity(screen.list).insert(ServerList::default());
    commands.entity(screen.add_button).observe(open_add_server);
    commands.trigger(ServersChanged {
        focus: None,
        error: None,
    });
}

/// Adds the row of a server to the list. Returns its Play button.
pub fn add_server_item(
    name: String,
    ip: String,
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    list: &mut ServerList,
    list_entity: Entity,
) -> Entity {
    info!("Adding server to list : name = {:?}, ip = {:?}", name, ip);

    let row = spawn_row(commands, asset_server, list_entity, &name, Some(&ip));
    commands
        .entity(row.play)
        .insert(MultiplayerButtonAction::Connect(row.row));
    commands
        .entity(row.delete)
        .insert(MultiplayerButtonAction::Delete(row.row));
    list.servers.push((row.row, ServerItem { name, ip }));
    row.play
}

/// Lists the saved servers again from `servers.ron`, and puts the focus on the Play button of
/// `focus`. This list has no place to show `error`: it goes to the log.
fn list_changed_servers(
    changed: On<ServersChanged>,
    mut lists: Query<(Entity, &mut ServerList)>,
    paths: Res<GameFolderPaths>,
    assets: Res<AssetServer>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let Ok((list_entity, mut list)) = lists.single_mut() else {
        return;
    };
    if let Some(error) = changed.error {
        warn!("{error}");
    }
    for (row, _) in list.servers.drain(..) {
        commands.entity(row).despawn();
    }

    let path = paths.game_folder_path.join(SERVER_LIST_SAVE_NAME);
    let servers = match read_servers(&path) {
        Ok(servers) => servers,
        Err(e) => {
            error!("Failed to read the server list from {path:?}, it will be left as it is: {e}");
            return;
        }
    };
    for srv in servers {
        let focused = changed.focus.as_ref() == Some(&srv.name);
        let play = add_server_item(
            srv.name,
            srv.ip,
            &mut commands,
            &assets,
            &mut list,
            list_entity,
        );
        if focused {
            focus.set(play, FocusCause::Navigated);
        }
    }
}

/// Opens the Add server dialog from the "Add server" button, with the listed servers.
fn open_add_server(
    click: On<Pointer<Click>>,
    lists: Query<&ServerList>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let others = lists.single().map(ServerList::items).unwrap_or_default();
    open_add_server_dialog(&mut commands, &assets, &focus, others);
}

pub fn multiplayer_action(
    interactions: Query<
        (&Interaction, &MultiplayerButtonAction),
        (Changed<Interaction>, With<Button>),
    >,
    list_query: Query<&ServerList>,
    paths: Res<GameFolderPaths>,
    mut commands: Commands,
    mut target_server: ResMut<TargetServer>,
    mut game_state: ResMut<NextState<GameState>>,
    mut menu_state: ResMut<NextState<MenuState>>,
) {
    let Ok(list) = list_query.single() else {
        return;
    };

    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            MultiplayerButtonAction::Connect(serv_entity) => {
                let Some((_, srv)) = list.servers.iter().find(|(e, _)| *e == serv_entity) else {
                    continue;
                };
                info!("Server : name={}, ip={}", srv.name, srv.ip);
                let Some(address) = parse_server_address(&srv.ip) else {
                    warn!(
                        "Not connecting to {:?}: invalid address {:?}",
                        srv.name, srv.ip
                    );
                    continue;
                };
                target_server.address = Some(address);
                target_server.state = TargetServerState::Initial;
                game_state.set(GameState::PreGameLoading);
                menu_state.set(MenuState::Disabled);
            }
            MultiplayerButtonAction::Delete(serv_entity) => {
                let Some((_, srv)) = list.servers.iter().find(|(e, _)| *e == serv_entity) else {
                    continue;
                };
                let path = paths.game_folder_path.join(SERVER_LIST_SAVE_NAME);
                let changed = match remove_server(&path, &srv.name) {
                    Ok(()) => ServersChanged {
                        focus: None,
                        error: None,
                    },
                    Err(e) => {
                        error!("Could not remove the server {:?}: {e}", srv.name);
                        ServersChanged {
                            focus: None,
                            error: Some("Could not remove the server"),
                        }
                    }
                };
                commands.trigger(changed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_an_ip_address_with_a_port() {
        assert_eq!(
            parse_server_address("127.0.0.1:8000"),
            Some("127.0.0.1:8000".parse().unwrap())
        );
    }

    #[test]
    fn rejects_host_names_and_incomplete_addresses() {
        assert_eq!(parse_server_address("localhost:8000"), None);
        assert_eq!(parse_server_address(""), None);
        assert_eq!(parse_server_address("1.2.3.4"), None);
    }
}
