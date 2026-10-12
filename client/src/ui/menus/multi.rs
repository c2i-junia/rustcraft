use super::list_screen::*;
use super::MenuState;
use crate::constants::SERVER_LIST_SAVE_NAME;
use crate::network::{TargetServer, TargetServerState};
use crate::state::GameState;
use bevy::prelude::*;
use bevy_simple_text_input::TextInputValue;
use ron::{from_str, ser::PrettyConfig};
use shared::GameFolderPaths;
use std::{fs, io, net::SocketAddr, path::Path};

/// Parses the address of a server as typed by the user: an IP address and a port, such as
/// `127.0.0.1:8000`. Host names are not resolved.
fn parse_server_address(text: &str) -> Option<SocketAddr> {
    text.parse().ok()
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ServerItem {
    pub name: String,
    pub ip: String,
}

#[derive(Component, Default)]
pub struct ServerList {
    /// The servers in the order they are shown, which is the order they are saved in.
    pub servers: Vec<(Entity, ServerItem)>,
    /// Set when the saved list could not be read. The list is then not saved either: that
    /// would replace a file the player may still repair by an empty one.
    pub unreadable: bool,
}

#[derive(Component)]
pub enum MultiplayerButtonAction {
    Add,
    Connect(Entity),
    Delete(Entity),
}

#[derive(Component)]
pub struct ServerIpInput;

#[derive(Component)]
pub struct ServerNameInput;

pub fn multiplayer_menu_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let screen = spawn_list_screen(
        &mut commands,
        &asset_server,
        &ListScreen {
            menu: MenuState::Multi,
            title: "Server list",
            inputs: &[
                InputField {
                    placeholder: "Server name",
                    ..default()
                },
                InputField {
                    placeholder: "Server IP",
                    ..default()
                },
            ],
            add_label: "Add server",
            stacked: false,
        },
    );
    commands.entity(screen.list).insert(ServerList::default());
    commands.entity(screen.inputs[0]).insert(ServerNameInput);
    commands.entity(screen.inputs[1]).insert(ServerIpInput);
    commands
        .entity(screen.add_button)
        .insert(MultiplayerButtonAction::Add);
}

pub fn add_server_item(
    name: String,
    ip: String,
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    list: &mut ServerList,
    list_entity: Entity,
) {
    info!("Adding server to list : name = {:?}, ip = {:?}", name, ip);

    let row = spawn_row(commands, asset_server, list_entity, &name, Some(&ip));
    commands
        .entity(row.play)
        .insert(MultiplayerButtonAction::Connect(row.row));
    commands
        .entity(row.delete)
        .insert(MultiplayerButtonAction::Delete(row.row));
    list.servers.push((row.row, ServerItem { name, ip }));
}

/// Reads the saved servers. A missing or empty file is an empty list.
fn read_servers(path: &Path) -> Result<Vec<ServerItem>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    from_str(&text).map_err(|e| e.to_string())
}

fn write_servers(path: &Path, servers: &[ServerItem]) -> Result<(), String> {
    let pretty_config = PrettyConfig::new()
        .with_depth_limit(3)
        .with_separate_tuple_members(true)
        .with_enumerate_arrays(true);
    let data = ron::ser::to_string_pretty(&servers, pretty_config).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

pub fn load_server_list(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut list_query: Query<(&mut ServerList, Entity)>,
    paths: Res<GameFolderPaths>,
) {
    let Ok((mut list, list_entity)) = list_query.single_mut() else {
        return;
    };

    let path = paths.game_folder_path.join(SERVER_LIST_SAVE_NAME);
    let servers = match read_servers(&path) {
        Ok(servers) => servers,
        Err(e) => {
            error!("Failed to read the server list from {path:?}, it will be left as it is: {e}");
            list.unreadable = true;
            return;
        }
    };

    // The local server is always offered, in first place, unless the list already has it.
    let localhost = ServerItem {
        name: "localhost".into(),
        ip: "127.0.0.1:8000".into(),
    };
    let default = (!servers.iter().any(|srv| srv.ip == localhost.ip)).then_some(localhost);
    for srv in default.into_iter().chain(servers) {
        add_server_item(
            srv.name,
            srv.ip,
            &mut commands,
            &assets,
            &mut list,
            list_entity,
        );
    }
}

pub fn save_server_list(list: Query<&ServerList>, game_folder_path: Res<GameFolderPaths>) {
    let Ok(list) = list.single() else {
        warn!("save_server_list: expected exactly one ServerList");
        return;
    };

    let save_path = game_folder_path
        .game_folder_path
        .join(SERVER_LIST_SAVE_NAME);
    if list.unreadable {
        warn!("Not saving the server list: {save_path:?} could not be read and is kept as it is");
        return;
    }
    let servers: Vec<ServerItem> = list.servers.iter().map(|(_, srv)| srv.clone()).collect();
    match write_servers(&save_path, &servers) {
        Ok(()) => info!("Server list saved to {save_path:?}"),
        Err(e) => error!("Failed to save the server list to {save_path:?}: {e}"),
    }
}

pub fn multiplayer_action(
    interactions: Query<
        (&Interaction, &MultiplayerButtonAction),
        (Changed<Interaction>, With<Button>),
    >,
    name_input: Query<&TextInputValue, (With<ServerNameInput>, Without<ServerIpInput>)>,
    ip_input: Query<&TextInputValue, (With<ServerIpInput>, Without<ServerNameInput>)>,
    mut list_query: Query<(Entity, &mut ServerList)>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut target_server: ResMut<TargetServer>,
    mut game_state: ResMut<NextState<GameState>>,
    mut menu_state: ResMut<NextState<MenuState>>,
) {
    let Ok((entity, mut list)) = list_query.single_mut() else {
        return;
    };

    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            MultiplayerButtonAction::Add => {
                let (Ok(name), Ok(ip)) = (name_input.single(), ip_input.single()) else {
                    continue;
                };
                if parse_server_address(&ip.0).is_none() {
                    warn!("Not adding server {:?}: invalid address {:?}", name.0, ip.0);
                    continue;
                }
                let (name, ip) = (name.0.clone(), ip.0.clone());
                add_server_item(name, ip, &mut commands, &asset_server, &mut list, entity);
            }
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
                commands.entity(entity).detach_children(&[serv_entity]);
                commands.entity(serv_entity).despawn();
                list.servers.retain(|(e, _)| *e != serv_entity);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::state::app::StatesPlugin;
    use std::path::PathBuf;

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

    /// A minimal app that runs the real list systems against a server file in its own folder.
    struct Menu {
        app: App,
        folder: PathBuf,
    }

    impl Menu {
        fn new(name: &str, saved: &[(&str, &str)]) -> Self {
            let saved: Vec<ServerItem> = saved
                .iter()
                .map(|(name, ip)| ServerItem {
                    name: name.to_string(),
                    ip: ip.to_string(),
                })
                .collect();
            let file = ron::ser::to_string(&saved).unwrap();
            Self::with_file(name, Some(&file))
        }

        /// Same, with the server file holding `file`, or missing for `None`.
        fn with_file(name: &str, file: Option<&str>) -> Self {
            let folder =
                std::env::temp_dir().join(format!("rustcraft-multi-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&folder);
            fs::create_dir_all(&folder).unwrap();
            if let Some(file) = file {
                fs::write(folder.join(SERVER_LIST_SAVE_NAME), file).unwrap();
            }

            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
                .init_asset::<Font>()
                .init_asset::<Image>()
                .init_state::<GameState>()
                .init_state::<MenuState>()
                .insert_resource(TargetServer {
                    address: None,
                    username: None,
                    session_token: None,
                    state: TargetServerState::Initial,
                })
                .insert_resource(GameFolderPaths {
                    game_folder_path: folder.clone(),
                    assets_folder_path: folder.join("assets"),
                });
            let world = app.world_mut();
            world.spawn(ServerList::default());
            world.spawn((ServerNameInput, TextInputValue(String::new())));
            world.spawn((ServerIpInput, TextInputValue(String::new())));
            world.spawn((MultiplayerButtonAction::Add, Button, Interaction::None));
            world.run_system_once(load_server_list).unwrap();
            Self { app, folder }
        }

        /// The names in the order the list shows them.
        fn shown(&mut self) -> Vec<String> {
            let list = self
                .app
                .world_mut()
                .query::<&ServerList>()
                .single(self.app.world())
                .unwrap();
            list.servers
                .iter()
                .map(|(_, srv)| srv.name.clone())
                .collect()
        }

        /// Saves the list with the real system and returns the names of the file.
        fn saved(&mut self) -> Vec<String> {
            self.app
                .world_mut()
                .run_system_once(save_server_list)
                .unwrap();
            let file = read_servers(&self.folder.join(SERVER_LIST_SAVE_NAME)).unwrap();
            file.into_iter().map(|srv| srv.name).collect()
        }

        /// Presses a button of the list: the real action system handles it.
        fn press(&mut self, button: Entity) {
            let world = self.app.world_mut();
            world.entity_mut(button).insert(Interaction::Pressed);
            world.run_system_once(multiplayer_action).unwrap();
            // A deleted row takes its buttons with it.
            if let Ok(mut button) = world.get_entity_mut(button) {
                button.insert(Interaction::None);
            }
        }

        fn add(&mut self, name: &str, ip: &str) {
            let world = self.app.world_mut();
            for (mut value, is_name) in world
                .query::<(&mut TextInputValue, Has<ServerNameInput>)>()
                .iter_mut(world)
            {
                value.0 = if is_name { name } else { ip }.to_string();
            }
            let add = world
                .query::<(Entity, &MultiplayerButtonAction)>()
                .iter(world)
                .find(|(_, action)| matches!(action, MultiplayerButtonAction::Add))
                .unwrap()
                .0;
            self.press(add);
        }

        /// Deletes the server at `index` in the shown order.
        fn delete(&mut self, index: usize) {
            let world = self.app.world_mut();
            let row = world.query::<&ServerList>().single(world).unwrap().servers[index].0;
            let delete = world
                .query::<(Entity, &MultiplayerButtonAction)>()
                .iter(world)
                .find(
                    |(_, action)| matches!(action, MultiplayerButtonAction::Delete(r) if *r == row),
                )
                .unwrap()
                .0;
            self.press(delete);
        }
    }

    impl Drop for Menu {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.folder);
        }
    }

    #[test]
    fn the_file_order_is_kept_on_load_and_save_and_new_servers_are_appended() {
        let mut menu = Menu::new(
            "order",
            &[
                ("zulu", "127.0.0.1:9001"),
                ("alpha", "127.0.0.1:9002"),
                ("localhost", "127.0.0.1:8000"),
                ("mike", "127.0.0.1:9003"),
            ],
        );
        let file = ["zulu", "alpha", "localhost", "mike"];
        assert_eq!(menu.shown(), file);
        assert_eq!(menu.saved(), file);

        menu.add("bravo", "127.0.0.1:9004");
        let appended = ["zulu", "alpha", "localhost", "mike", "bravo"];
        assert_eq!(menu.shown(), appended);
        assert_eq!(menu.saved(), appended);
    }

    #[test]
    fn deleting_the_middle_server_keeps_the_order_of_the_others() {
        let mut menu = Menu::new(
            "delete",
            &[
                ("zulu", "127.0.0.1:8000"),
                ("alpha", "127.0.0.1:9002"),
                ("mike", "127.0.0.1:9003"),
            ],
        );
        menu.delete(1);
        assert_eq!(menu.shown(), ["zulu", "mike"]);
        assert_eq!(menu.saved(), ["zulu", "mike"]);
    }

    #[test]
    fn localhost_is_added_first_when_missing_and_only_once() {
        let mut menu = Menu::new("default", &[("zulu", "127.0.0.1:9001")]);
        assert_eq!(menu.shown(), ["localhost", "zulu"]);
        assert_eq!(menu.saved(), ["localhost", "zulu"]);

        // Loading what was saved does not add it a second time.
        let mut again = Menu::new(
            "default-again",
            &[("localhost", "127.0.0.1:8000"), ("zulu", "1.2.3.4:5")],
        );
        assert_eq!(again.shown(), ["localhost", "zulu"]);
        assert_eq!(again.saved(), ["localhost", "zulu"]);
    }

    #[test]
    fn an_empty_or_missing_server_file_is_an_empty_list() {
        let path = std::env::temp_dir().join(format!(
            "rustcraft-servers-empty-{}.ron",
            std::process::id()
        ));
        assert!(read_servers(&path).unwrap().is_empty());
        fs::write(&path, " \n").unwrap();
        assert!(read_servers(&path).unwrap().is_empty());
        fs::write(&path, "not ron").unwrap();
        assert!(read_servers(&path).is_err());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn a_missing_server_file_starts_a_list_that_is_saved() {
        let mut menu = Menu::with_file("missing", None);
        assert_eq!(menu.shown(), ["localhost"]);
        assert_eq!(menu.saved(), ["localhost"]);
    }

    #[test]
    fn a_server_file_that_cannot_be_read_is_never_overwritten() {
        let mut menu = Menu::with_file("unreadable", Some("not ron"));
        assert!(menu.shown().is_empty());

        menu.add("bravo", "127.0.0.1:9004");
        menu.app
            .world_mut()
            .run_system_once(save_server_list)
            .unwrap();

        let file = fs::read_to_string(menu.folder.join(SERVER_LIST_SAVE_NAME)).unwrap();
        assert_eq!(file, "not ron");
    }
}
