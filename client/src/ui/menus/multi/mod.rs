//! The Multiplayer screen: the saved servers as cards with their status, and a detail pane for
//! the selected one with Join and Remove. Refresh (F5) and Add server (Ctrl+N) sit in the title
//! bar.
//!
//! The cards are always built again from `servers.ron` once a server was added or removed; their
//! statuses live on the screen's root (`status::StatusBoard`), asked on threads, and never wait
//! for the network on the frame. Joining resolves the address on a thread too.

pub mod dialogs;
pub mod status;

use std::io;
use std::net::SocketAddr;

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    ecs::system::SystemParam,
    input::{keyboard::KeyboardInput, ButtonState},
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        FocusCause, FocusedInput, InputFocus, InputFocusVisible,
    },
    prelude::*,
    ui::{InteractionDisabled, Selected},
    ui_widgets::{observe, Activate},
};
use shared::GameFolderPaths;

use super::background::menu_background;
use super::servers::{read_servers, ServerItem};
use super::{neighbour, text, MenuState};
use crate::constants::SERVER_LIST_SAVE_NAME;
use crate::network::address::{parse_server_address, AddressResolver};
use crate::network::status::{InFlight, ServerStatus, StatusProbe};
use crate::network::{TargetServer, TargetServerState};
use crate::state::GameState;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::theme::{
    BEVEL_WIDTH, DANGER, FONT_L, FONT_M, FONT_S, FONT_XL, MUTED, PANEL, SPACE_L, SPACE_M, SPACE_S,
    SPACE_XL, SPACE_XS, TEXT,
};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::dialog::{dialog_open, CloseRequest, DialogRoot};
use crate::ui::widgets::focus::ArrowNav;
use crate::ui::widgets::hints::{hint_bar, Hint, HINT_BAR_HEIGHT};
use crate::ui::widgets::list::{
    cards_of, spawn_list_card, spawn_scroll_list, CardSelected, ListCard,
};
use crate::ui::widgets::raised_bevel;
use dialogs::{open_add_server_dialog, open_remove_server_dialog, ServersChanged};
use status::{
    can_join, card_status_node, pane_status, ping_every_server, show_card_status, status_dot,
    StatusBoard,
};

/// The title of the screen.
pub const SERVERS_TITLE: &str = "Servers";

/// A server card, and the server it shows.
#[derive(Component)]
pub struct ServerCard(pub ServerItem);

/// The list of the server cards: the scroll area they sit in.
#[derive(Component)]
pub struct ServerList;

/// The detail pane of the selected server, hidden while there is no server.
#[derive(Component)]
pub struct ServerDetail;

/// What the selected server shows in the detail pane, above its buttons.
#[derive(Component)]
struct ServerFacts;

/// A button of the detail pane, which acts on the selected server.
#[derive(Component, Clone, Copy)]
pub enum ServerAction {
    Join,
    Remove,
}

/// The line under the buttons of the detail pane: why an action failed (`DANGER`), or the host a
/// join is looking up (`MUTED`); hidden while it is empty.
#[derive(Component)]
pub struct ServerError;

/// The Add server button of the title bar.
#[derive(Component)]
pub struct AddServerButton;

/// The Refresh button of the title bar.
#[derive(Component)]
pub struct RefreshButton;

/// The Back button, which has the focus while the list cannot be read.
#[derive(Component)]
struct BackButton;

/// A join waiting for its address to resolve, on the screen's root: leaving the screen drops it.
#[derive(Component)]
struct PendingJoin {
    host: String,
    address: InFlight<io::Result<SocketAddr>>,
}

/// Which key hints the hint bar of the screen shows.
#[derive(Component, PartialEq, Eq)]
enum MultiHints {
    /// A card has the focus.
    Cards,
    /// Anything else has it, or there is no server.
    Elsewhere,
}

const REFRESH_HINT: Hint = Hint {
    keys: "F5",
    action: "Refresh",
};
const BACK_HINT: Hint = Hint {
    keys: "Esc",
    action: "Back",
};

/// The key hints while a card has the focus.
pub const CARD_HINTS: [Hint; 5] = [
    Hint {
        keys: "↑↓",
        action: "Select",
    },
    Hint {
        keys: "Enter",
        action: "Join",
    },
    Hint {
        keys: "Del",
        action: "Remove",
    },
    REFRESH_HINT,
    BACK_HINT,
];

/// The key hints while anything else has the focus.
pub const SCREEN_HINTS: [Hint; 5] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Select",
    },
    Hint {
        keys: "Ctrl+N",
        action: "Add server",
    },
    REFRESH_HINT,
    BACK_HINT,
];

/// The width of the detail pane.
const DETAIL_WIDTH: f32 = 280.0;

/// The line of the screen while `servers.ron` cannot be read.
const UNREADABLE: &str = "Cannot read servers.ron";

/// The Multiplayer screen and its dialogs.
pub fn multi_plugin(app: &mut App) {
    dialogs::plugin(app);
    status::plugin(app);
    app.init_resource::<StatusProbe>()
        .init_resource::<AddressResolver>()
        .add_systems(OnEnter(MenuState::Multi), multi_setup)
        .add_systems(
            Update,
            (
                // after Ctrl+N, which drops a join along with opening its dialog
                finish_join
                    .run_if(not(dialog_open))
                    .after(dialogs::open_add_server_on_ctrl_n),
                follow_focus_with_hints,
            )
                .run_if(in_state(MenuState::Multi)),
        )
        .add_observer(show_changed_servers)
        .add_observer(join_or_remove_on_key)
        .add_observer(act_on_selected_server);
}

/// Spawns the screen, then lists the servers, which asks them for their status.
fn multi_setup(mut commands: Commands, assets: Res<AssetServer>) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let root = commands
        .spawn((
            menu_background(&assets),
            DespawnOnExit(MenuState::Multi),
            TabGroup::new(0),
            StatusBoard::default(),
        ))
        .id();
    let content = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    left: px(SPACE_XL),
                    right: px(SPACE_XL),
                    top: px(SPACE_XL),
                    bottom: px(HINT_BAR_HEIGHT + SPACE_XL),
                },
                row_gap: px(SPACE_M),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Node {
            align_items: AlignItems::Center,
            column_gap: px(SPACE_S),
            ..default()
        },
        ChildOf(content),
        children![
            text(SERVERS_TITLE, FONT_XL, TEXT, &font),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            (
                button("Refresh", ButtonVariant::Default, font.clone()),
                RefreshButton,
                observe(refresh),
            ),
            (
                button("Add server", ButtonVariant::Default, font.clone()),
                AddServerButton,
                observe(open_add_server),
            ),
        ],
    ));
    let body = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_height: px(0),
                column_gap: px(SPACE_L),
                ..default()
            },
            ChildOf(content),
        ))
        .id();
    let scroll = spawn_scroll_list(&mut commands);
    commands.entity(scroll.frame).insert(ChildOf(body));
    commands
        .entity(scroll.list)
        .insert((
            ServerList,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::ListBox)),
            AccessibleLabel::new(SERVERS_TITLE),
        ))
        .observe(show_selected_server);
    spawn_detail_pane(&mut commands, body, &font);
    commands.spawn((
        Node::default(),
        ChildOf(content),
        children![(
            button("Back", ButtonVariant::Default, font.clone()),
            BackButton,
            observe(
                |_: On<Activate>, mut menu_state: ResMut<NextState<MenuState>>| {
                    menu_state.set(MenuState::Main);
                }
            ),
        )],
    ));
    commands.spawn((
        hint_bar(&SCREEN_HINTS, font, assets.load(MONO_FONT_PATH)),
        MultiHints::Elsewhere,
        ChildOf(root),
    ));
    commands.trigger(ServersChanged {
        focus: None,
        error: None,
    });
}

/// Spawns the detail pane in `body`: the facts of the selected server, Join, Remove, and the
/// line of a failure.
fn spawn_detail_pane(commands: &mut Commands, body: Entity, font: &Handle<Font>) {
    let pane = commands
        .spawn((
            ServerDetail,
            Node {
                width: px(DETAIL_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(px(SPACE_L)),
                border: UiRect::all(px(BEVEL_WIDTH)),
                row_gap: px(SPACE_L),
                ..default()
            },
            BackgroundColor(PANEL),
            raised_bevel(),
            ChildOf(body),
        ))
        .id();
    commands.spawn((
        ServerFacts,
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_XS),
            min_width: px(0),
            ..default()
        },
        ChildOf(pane),
    ));
    let actions = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_S),
                ..default()
            },
            ChildOf(pane),
        ))
        .id();
    commands.spawn((
        ArrowNav,
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_S),
            ..default()
        },
        ChildOf(actions),
        children![
            (
                button("Join", ButtonVariant::Primary, font.clone()),
                ServerAction::Join,
            ),
            (
                button("Remove", ButtonVariant::Danger, font.clone()),
                ServerAction::Remove,
            ),
        ],
    ));
    commands.spawn((
        ServerError,
        text("", FONT_S, DANGER, font),
        Node {
            display: Display::None,
            ..default()
        },
        ChildOf(actions),
    ));
}

/// Despawns what `list` shows and shows the empty state: `title`, `line` under it, and `error`
/// under them; the detail pane, which shows it otherwise, is hidden.
fn show_empty(
    commands: &mut Commands,
    list: Entity,
    (title, line): (&str, &str),
    error: Option<&str>,
    font: &Handle<Font>,
) {
    commands.entity(list).despawn_related::<Children>();
    let empty = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(SPACE_S),
                ..default()
            },
            ChildOf(list),
            children![
                text(title, FONT_L, TEXT, font),
                text(line, FONT_S, MUTED, font),
            ],
        ))
        .id();
    if let Some(message) = error {
        commands.spawn((text(message, FONT_S, DANGER, font), ChildOf(empty)));
    }
}

/// Despawns the cards and spawns one per server, in order, with what `board` knows of it;
/// returns them. Shows the empty state when there is none, with `error` under it.
fn show_servers(
    commands: &mut Commands,
    list: Entity,
    servers: &[ServerItem],
    error: Option<&str>,
    board: &StatusBoard,
    font: &Handle<Font>,
) -> Vec<Entity> {
    if servers.is_empty() {
        let empty = ("No servers yet", "Add one with Add server (Ctrl+N).");
        show_empty(commands, list, empty, error, font);
        return Vec::new();
    }
    commands.entity(list).despawn_related::<Children>();
    servers
        .iter()
        .map(|server| {
            let card = spawn_list_card(commands, list, &server.name);
            let status = board.status(&server.ip);
            // each text in a box of its own: a text that shrinks itself adds its height at the
            // narrowest, a letter a line, to the content size of the list
            let boxed = |content| {
                (
                    Node {
                        min_width: px(0),
                        ..default()
                    },
                    children![content],
                )
            };
            commands.entity(card).insert((
                ServerCard(server.clone()),
                children![
                    status_dot(&status),
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            min_width: px(0),
                            row_gap: px(SPACE_XS),
                            ..default()
                        },
                        children![
                            boxed(text(&server.name, FONT_M, TEXT, font)),
                            boxed(text(&server.ip, FONT_S, MUTED, font)),
                        ],
                    ),
                ],
            ));
            let side = commands.spawn((card_status_node(), ChildOf(card))).id();
            show_card_status(commands, side, &status, font);
            card
        })
        .collect()
}

/// Shows `server`, of the status `status`, in the detail pane, in place of what it showed.
fn show_detail(
    commands: &mut Commands,
    facts: Entity,
    server: &ServerItem,
    status: &ServerStatus,
    font: &Handle<Font>,
) {
    commands.entity(facts).despawn_related::<Children>();
    let (headline, color, line) = pane_status(status);
    let mut show = |content: &str, size: f32, color: Color| {
        commands
            .spawn((text(content, size, color, font), ChildOf(facts)))
            .id()
    };
    show(&server.name, FONT_L, TEXT);
    show(&server.ip, FONT_S, MUTED);
    let headline = show(&headline, FONT_M, color);
    if !line.is_empty() {
        show(&line, FONT_S, MUTED);
    }
    commands.entity(headline).insert(Node {
        margin: UiRect::top(px(SPACE_S)),
        ..default()
    });
}

/// Enables Join in the detail pane for a server of `status`, or disables it.
fn allow_join(commands: &mut Commands, status: &ServerStatus) {
    let allowed = can_join(status);
    commands.queue(move |world: &mut World| {
        let mut actions = world.query::<(Entity, &ServerAction)>();
        let joins: Vec<Entity> = actions
            .iter(world)
            .filter(|(_, action)| matches!(action, ServerAction::Join))
            .map(|(entity, _)| entity)
            .collect();
        for join in joins {
            let mut join = world.entity_mut(join);
            if allowed {
                join.remove::<InteractionDisabled>();
            } else if !join.contains::<InteractionDisabled>() {
                join.insert(InteractionDisabled);
            }
        }
    });
}

/// The line under the buttons of the detail pane.
type ErrorLines<'w, 's> = Query<
    'w,
    's,
    (&'static mut Text, &'static mut TextColor, &'static mut Node),
    (With<ServerError>, Without<ServerDetail>),
>;

/// Shows `message` in its colour under the buttons of the detail pane, or hides the line for
/// `None`.
fn show_server_error(errors: &mut ErrorLines, message: Option<(String, Color)>) {
    for (mut text, mut color, mut node) in errors.iter_mut() {
        node.display = if message.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        let (content, tint) = message.clone().unwrap_or((String::new(), DANGER));
        text.0 = content;
        color.0 = tint;
    }
}

/// The names of the server cards, from the top of the list.
fn card_names(
    lists: &Query<Entity, With<ServerList>>,
    children: &Query<&Children>,
    cards: &Query<&ServerCard>,
) -> Vec<String> {
    lists
        .iter()
        .flat_map(|list| cards_of(list, children))
        .filter_map(|card| cards.get(card).ok())
        .map(|card| card.0.name.clone())
        .collect()
}

/// Where the focus goes once the cards were built again. A dialog still open (not closing) keeps
/// it.
#[derive(SystemParam)]
struct ListFocus<'w, 's> {
    focus: ResMut<'w, InputFocus>,
    visible: ResMut<'w, InputFocusVisible>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    open_dialogs: Query<'w, 's, (), (With<DialogRoot>, Without<CloseRequest>)>,
}

impl ListFocus<'_, '_> {
    /// Puts the focus on `entity`; on a card reached by the keyboard, with its ring.
    fn set(&mut self, entity: Entity, card: bool) {
        if !self.open_dialogs.is_empty() {
            return;
        }
        self.focus.set(entity, FocusCause::Navigated);
        // reached by the keyboard, the card shows its ring; a click leaves it hidden
        let typed = self.keys.get_just_pressed().next().is_some()
            || self.keys.get_just_released().next().is_some();
        if card && typed {
            self.visible.0 = true;
        }
    }
}

/// Lists the servers of `servers.ron` again, asks those the board does not know for their
/// status, selects and focuses the card of `focus`, else the first card, and shows `error` under
/// the buttons of the detail pane. Without a server, the detail pane hides, its buttons leave the
/// tab order and Add server takes the focus; while the file cannot be read, Add server and
/// Refresh are disabled and Back takes it.
#[allow(clippy::too_many_arguments)]
fn show_changed_servers(
    changed: On<ServersChanged>,
    lists: Query<Entity, With<ServerList>>,
    mut boards: Query<&mut StatusBoard>,
    mut panes: Query<&mut Node, With<ServerDetail>>,
    mut actions: Query<&mut TabIndex, With<ServerAction>>,
    facts: Query<Entity, With<ServerFacts>>,
    title_buttons: Query<
        (Entity, Has<AddServerButton>),
        Or<(With<AddServerButton>, With<RefreshButton>)>,
    >,
    back: Query<Entity, With<BackButton>>,
    mut errors: ErrorLines,
    mut list_focus: ListFocus,
    paths: Res<GameFolderPaths>,
    probe: Res<StatusProbe>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let (Ok(list), Ok(mut board)) = (lists.single(), boards.single_mut()) else {
        return;
    };
    let font: Handle<Font> = assets.load(FONT_PATH);
    let path = paths.game_folder_path.join(SERVER_LIST_SAVE_NAME);
    let read = read_servers(&path);
    let servers = read.as_deref().unwrap_or_default();
    // a server gone, or added again, is asked afresh: no stale status, no orphan probe
    board.keep_only(servers.iter().map(|server| server.ip.as_str()));
    for server in servers {
        if !board.knows(&server.ip) {
            board.ping(&server.ip, *probe);
        }
    }
    let cards = match &read {
        Ok(servers) => show_servers(&mut commands, list, servers, changed.error, &board, &font),
        Err(e) => {
            error!("Cannot read the server list {path:?}, it will be left as it is: {e}");
            let unreadable = (UNREADABLE, "Fix or delete the file, then come back.");
            show_empty(&mut commands, list, unreadable, changed.error, &font);
            Vec::new()
        }
    };
    let mut add_server = None;
    for (button, is_add) in &title_buttons {
        if read.is_ok() {
            commands.entity(button).remove::<InteractionDisabled>();
        } else {
            commands.entity(button).insert(InteractionDisabled);
        }
        if is_add {
            add_server = Some(button);
        }
    }
    let chosen = changed
        .focus
        .as_ref()
        .and_then(|name| servers.iter().position(|server| &server.name == name))
        .unwrap_or(0);
    let shown = match (cards.get(chosen), facts.single()) {
        // the list sends no `CardSelected` for a card selected here: the pane is filled now
        (Some(&card), Ok(facts)) => {
            commands.entity(card).insert((Selected, TabIndex(0)));
            list_focus.set(card, true);
            let status = board.status(&servers[chosen].ip);
            show_detail(&mut commands, facts, &servers[chosen], &status, &font);
            allow_join(&mut commands, &status);
            true
        }
        _ => {
            let fallback = if read.is_ok() {
                add_server
            } else {
                back.single().ok()
            };
            if let Some(fallback) = fallback {
                list_focus.set(fallback, false);
            }
            false
        }
    };
    for mut pane in &mut panes {
        pane.display = if shown { Display::Flex } else { Display::None };
    }
    // a hidden button would still take Tab
    for mut tab_index in &mut actions {
        tab_index.set_if_neq(TabIndex(if shown { 0 } else { -1 }));
    }
    show_server_error(
        &mut errors,
        changed.error.map(|error| (error.to_string(), DANGER)),
    );
}

/// Shows the server of the card just selected in the detail pane.
fn show_selected_server(
    selected: On<CardSelected>,
    cards: Query<&ServerCard>,
    boards: Query<&StatusBoard>,
    facts: Query<Entity, With<ServerFacts>>,
    mut errors: ErrorLines,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let (Ok(card), Ok(board), Ok(facts)) =
        (cards.get(selected.card), boards.single(), facts.single())
    else {
        return;
    };
    let status = board.status(&card.0.ip);
    let font = assets.load(FONT_PATH);
    show_detail(&mut commands, facts, &card.0, &status, &font);
    allow_join(&mut commands, &status);
    cancel_join(&mut commands);
    show_server_error(&mut errors, None);
}

/// Starts joining `server`, unless its status says it cannot be joined: its address resolves on
/// a thread, and `finish_join` hands it over. The screen's `root` holds the join, and the pane
/// says which host is looked up while it is not an IP address.
fn start_join(
    commands: &mut Commands,
    root: Entity,
    server: &ServerItem,
    status: &ServerStatus,
    resolver: AddressResolver,
    errors: &mut ErrorLines,
) {
    if !can_join(status) {
        return;
    }
    let Ok(address) = parse_server_address(&server.ip) else {
        return;
    };
    let looking_up = (!address.is_ip()).then(|| (format!("Looking up {}...", address.host), MUTED));
    show_server_error(errors, looking_up);
    let host = address.host.clone();
    commands.entity(root).insert(PendingJoin {
        host,
        address: InFlight::spawn(move || (resolver.0)(&address)),
    });
}

/// Drops the join still looking up its address, if there is one, and the line that says so:
/// the player has moved on, to another server or to a dialog.
pub(super) fn cancel_join(commands: &mut Commands) {
    commands.queue(|world: &mut World| {
        let mut pending = world.query_filtered::<Entity, With<PendingJoin>>();
        let roots: Vec<Entity> = pending.iter(world).collect();
        if roots.is_empty() {
            return;
        }
        for root in roots {
            world.entity_mut(root).remove::<PendingJoin>();
        }
        let mut lines = world.query_filtered::<(&mut Text, &mut Node), With<ServerError>>();
        for (mut text, mut node) in lines.iter_mut(world) {
            text.0.clear();
            node.display = Display::None;
        }
    });
}

/// Hands the address of a join over to the connection once it is resolved, or says the host
/// was not found.
fn finish_join(
    pending: Query<(Entity, &PendingJoin)>,
    mut errors: ErrorLines,
    mut target: ResMut<TargetServer>,
    mut game: ResMut<NextState<GameState>>,
    mut menu: ResMut<NextState<MenuState>>,
    mut commands: Commands,
) {
    for (root, join) in &pending {
        let Some(result) = join.address.take() else {
            continue;
        };
        commands.entity(root).remove::<PendingJoin>();
        match result {
            Ok(address) => join_server(address, &mut target, &mut game, &mut menu),
            Err(e) => {
                warn!("Cannot find the server {:?}: {e}", join.host);
                let message = format!("Cannot find {}", join.host);
                show_server_error(&mut errors, Some((message, DANGER)));
            }
        }
    }
}

/// Sets `address` as the server to connect to and leaves the menus for the connecting screen.
pub fn join_server(
    address: SocketAddr,
    target: &mut TargetServer,
    game: &mut NextState<GameState>,
    menu: &mut NextState<MenuState>,
) {
    info!("Joining the server at {address}");
    target.address = Some(address);
    target.state = TargetServerState::Initial;
    game.set(GameState::PreGameLoading);
    menu.set(MenuState::Disabled);
}

/// Enter on a focused card joins its server, unless Control is held; Delete asks to remove it.
#[allow(clippy::too_many_arguments)]
fn join_or_remove_on_key(
    mut event: On<FocusedInput<KeyboardInput>>,
    cards: Query<&ServerCard>,
    roots: Query<(Entity, &StatusBoard)>,
    lists: Query<Entity, With<ServerList>>,
    children: Query<&Children>,
    mut errors: ErrorLines,
    resolver: Res<AddressResolver>,
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    // the event bubbles: only the step on the card itself is handled
    let Ok(card) = cards.get(event.focused_entity) else {
        return;
    };
    let input = &event.input;
    if input.state != ButtonState::Pressed || input.repeat {
        return;
    }
    let control = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    match input.key_code {
        KeyCode::Enter if !control => {
            if let Ok((root, board)) = roots.single() {
                let status = board.status(&card.0.ip);
                start_join(
                    &mut commands,
                    root,
                    &card.0,
                    &status,
                    *resolver,
                    &mut errors,
                );
            }
        }
        KeyCode::Delete => {
            let next = neighbour(&card_names(&lists, &children, &cards), &card.0.name);
            open_remove_server_dialog(&mut commands, &assets, &focus, &card.0, next);
        }
        _ => return,
    }
    event.propagate(false);
}

/// Carries out the button of the detail pane that was activated, on the selected server.
#[allow(clippy::too_many_arguments)]
fn act_on_selected_server(
    activate: On<Activate>,
    actions: Query<&ServerAction>,
    selected_cards: Query<&ServerCard, With<Selected>>,
    roots: Query<(Entity, &StatusBoard)>,
    lists: Query<Entity, With<ServerList>>,
    children: Query<&Children>,
    cards: Query<&ServerCard>,
    mut errors: ErrorLines,
    resolver: Res<AddressResolver>,
    focus: Res<InputFocus>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let (Ok(action), Ok(card)) = (actions.get(activate.entity), selected_cards.single()) else {
        return;
    };
    match action {
        ServerAction::Join => {
            if let Ok((root, board)) = roots.single() {
                let status = board.status(&card.0.ip);
                start_join(
                    &mut commands,
                    root,
                    &card.0,
                    &status,
                    *resolver,
                    &mut errors,
                );
            }
        }
        ServerAction::Remove => {
            let next = neighbour(&card_names(&lists, &children, &cards), &card.0.name);
            open_remove_server_dialog(&mut commands, &assets, &focus, &card.0, next);
        }
    }
}

/// Asks every listed server again, from the button of the title bar.
fn refresh(
    _: On<Activate>,
    mut boards: Query<&mut StatusBoard>,
    cards: Query<&ServerCard>,
    probe: Res<StatusProbe>,
) {
    if let Ok(mut board) = boards.single_mut() {
        ping_every_server(&mut board, &cards, *probe);
    }
}

/// Opens the Add server dialog from the button of the title bar, with the listed servers.
fn open_add_server(
    _: On<Activate>,
    cards: Query<&ServerCard>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    let others = cards.iter().map(|card| card.0.clone()).collect();
    open_add_server_dialog(&mut commands, &assets, &focus, others);
}

/// Keeps the hint bar on the hints of the cards while a card has the focus, and on those of the
/// screen otherwise.
fn follow_focus_with_hints(
    focus: Res<InputFocus>,
    cards: Query<(), With<ListCard>>,
    bars: Query<(Entity, &MultiHints, &ChildOf)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let wanted = if focus.get().is_some_and(|entity| cards.contains(entity)) {
        MultiHints::Cards
    } else {
        MultiHints::Elsewhere
    };
    let Ok((bar, shown, child_of)) = bars.single() else {
        return;
    };
    if *shown == wanted {
        return;
    }
    let hints: &[Hint] = match wanted {
        MultiHints::Cards => &CARD_HINTS,
        MultiHints::Elsewhere => &SCREEN_HINTS,
    };
    commands.entity(bar).despawn();
    commands.spawn((
        hint_bar(hints, assets.load(FONT_PATH), assets.load(MONO_FONT_PATH)),
        wanted,
        ChildOf(child_of.parent()),
    ));
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;

    use super::*;
    use crate::network::status::InFlight;

    #[test]
    fn a_join_resolved_after_the_screen_closed_hands_nothing_over() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<GameState>()
            .init_state::<MenuState>()
            .insert_resource(TargetServer {
                address: None,
                username: None,
                session_token: None,
                state: TargetServerState::Initial,
            })
            .add_systems(Update, finish_join);
        let address: SocketAddr = "10.0.0.1:8000".parse().unwrap();
        let pending = || PendingJoin {
            host: "10.0.0.1".to_string(),
            address: InFlight::ready(Ok(address)),
        };
        let root = app.world_mut().spawn(pending()).id();
        app.world_mut().despawn(root);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<TargetServer>().address, None);
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Menu
        );

        // still on the screen, the same join is handed over
        app.world_mut().spawn(pending());
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<TargetServer>().address,
            Some(address)
        );
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::PreGameLoading
        );
    }
}
