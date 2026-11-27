//! The Hosted worlds tab of the Multiplayer screen: the worlds the account owns or is a member
//! of, from the hub, as cards with their state, and a detail pane for the selected one with
//! Join, Members, Stop and Delete as the player's role allows. Logged out, the tab offers the
//! Account dialog instead.
//!
//! Every call to the hub runs on a thread (`HubCalls`, on the screen's root): the screen stays
//! usable, and shows the answers as they come. The cards are built again from each answer of
//! `GET /v1/realms`.

use crate::i18n::tr;
use std::time::{Duration, Instant};

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input::{keyboard::KeyboardInput, ButtonState},
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        AutoFocus, FocusedInput, InputFocus,
    },
    prelude::*,
    ui::{InteractionDisabled, Selected},
    ui_widgets::{observe, Activate},
};
use rustcraft_protocol::api::{Me, MemberRole, Realm, RealmList, RealmState, Role as HubRole};
use uuid::Uuid;

use super::super::background::menu_background;
use super::super::{
    follow_focus_with_hints, list_hint_bar, text, ListEmpty, ListFocus, ListHints, MenuState,
};
use super::hosted_dialogs::{
    self, open_create_dialog, open_delete_dialog, open_members_dialog, open_stop_dialog,
    refill_members, HostedDialog,
};
use super::SERVERS_TITLE;
use crate::account::file::AccountFile;
use crate::account::hub::{hub_message, HubClient, HubError};
use crate::account::Account;
use crate::network::address::AddressResolver;
use crate::network::hosted::{join_hosted_world, typed_address, HostedJoin};
use crate::network::session::SessionEnd;
use crate::network::status::{InFlight, StatusProbe};
use crate::network::TargetServer;
use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::menus::account::{end_revoked_session, NextDialog, Screen};
use crate::ui::menus::modal_open;
use crate::ui::theme;
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::dialog::dismiss_dialog;
use crate::ui::widgets::focus::ArrowNav;
use crate::ui::widgets::hints::{Hint, HINT_BAR_HEIGHT};
use crate::ui::widgets::list::{spawn_list_card, spawn_scroll_list, CardSelected, WheelStep};
use crate::ui::widgets::raised_bevel;
use crate::ui::widgets::tabs::{spawn_tabs, TabChange, Tabs};
use crate::ui::widgets::text_field::text_field_focused;
use crate::ui::widgets::toast::{show_toast, ToastKind};

/// The title of the tab.
pub const HOSTED_TITLE: &str = "Hosted worlds";
/// What the tab says while the player is logged out.
pub const LOGGED_OUT: &str = "Log in to see your hosted worlds.";
/// The button of the title bar that opens the form of a new world.
pub const CREATE_WORLD: &str = "Create a world";

/// How long the answer of the hub is waited for. The hub client gives up after 10 s, so a call
/// still there after this one has lost its thread.
const CALL_DEADLINE: Duration = Duration::from_secs(30);
/// The width of the detail pane, as on the Servers tab.
const DETAIL_WIDTH: f32 = 280.0;
/// The side of the state dot of a card, as on the Servers tab.
const DOT_SIDE: f32 = 10.0;

/// The root of the screen: what the hub said, and the calls waiting for its answer.
#[derive(Component, Default)]
pub struct HostedBoard {
    /// The account the board was filled for: another one (or none) fills it again.
    account: Option<Uuid>,
    me: Option<Me>,
    realms: RealmList,
    /// Why the worlds could not be listed, while none is.
    error: Option<String>,
    /// Whether the worlds were listed once.
    listed: bool,
}

/// Where the screen shows its parts, on its root.
#[derive(Component)]
struct HostedParts {
    /// The row under the title bar: the list and the pane, or what the tab says instead.
    body: Entity,
    /// The buttons of the title bar after the tabs.
    actions: Entity,
}

/// The calls to the hub waiting for their answer, on the screen's root: leaving the screen drops
/// them, and their answers.
#[derive(Component, Default)]
pub struct HubCalls(Vec<Call>);

/// A call to the hub waiting for its answer.
struct Call {
    sent: Instant,
    /// Whether it lists the worlds.
    listing: bool,
    /// The Members dialog that waits for it: usable again once it is answered, lost or
    /// forgotten.
    dialog: Option<Entity>,
    answer: InFlight<Answer>,
}

/// What a call to the hub answered.
pub enum Answer {
    /// `GET /v1/me` then `GET /v1/realms`; the card of `focus` is selected once they are shown.
    Listed {
        result: Result<(Me, RealmList), HubError>,
        focus: Option<Uuid>,
    },
    /// `POST /v1/realms`, from the form dialog `form`.
    Created {
        result: Result<Realm, HubError>,
        form: Entity,
    },
    /// A change to a world or to its members. Done, `done` is said in a toast (unless empty)
    /// and the worlds are listed again, `focus` selected; failed, the hub's message shows under
    /// the field `field`, else in a toast.
    Changed {
        result: Result<(), HubError>,
        done: String,
        focus: Option<Uuid>,
        field: Option<Entity>,
    },
    /// The call could not start: there was no thread for it.
    Lost,
}

impl Answer {
    /// Whether the hub refused the session of the account.
    fn unauthorized(&self) -> bool {
        match self {
            Answer::Listed { result, .. } => result.as_ref().is_err_and(HubError::is_unauthorized),
            Answer::Created { result, .. } => result.as_ref().is_err_and(HubError::is_unauthorized),
            Answer::Changed { result, .. } => result.as_ref().is_err_and(HubError::is_unauthorized),
            Answer::Lost => false,
        }
    }
}

impl HubCalls {
    /// Runs `call` on a thread, which the Members dialog `dialog` waits for; its answer is
    /// taken by `take_answers`.
    pub fn spawn(
        &mut self,
        dialog: Option<Entity>,
        call: impl FnOnce() -> Answer + Send + 'static,
    ) {
        self.push(false, dialog, call);
    }

    fn push(
        &mut self,
        listing: bool,
        dialog: Option<Entity>,
        call: impl FnOnce() -> Answer + Send + 'static,
    ) {
        self.0.push(Call {
            sent: Instant::now(),
            listing,
            dialog,
            answer: InFlight::spawn(Answer::Lost, call),
        });
    }

    /// Whether the worlds are being listed.
    fn listing(&self) -> bool {
        self.0.iter().any(|call| call.listing)
    }
}

/// The cards of the list, and the world each shows.
#[derive(Component)]
pub struct HostedCard(pub Uuid);

/// The list of the cards.
#[derive(Component)]
pub struct HostedList;

/// The detail pane of the selected world.
#[derive(Component)]
pub struct HostedPane;

/// A button of the detail pane, on the world `.0`.
#[derive(Component, Clone, Copy)]
pub struct PaneAction(pub Uuid, pub PaneKind);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaneKind {
    Join,
    Members,
    Stop,
    Delete,
}

/// The Refresh button of the title bar.
#[derive(Component)]
struct RefreshButton;

/// The tab bar of the screen.
#[derive(Component)]
struct HostedTabs;

/// What the player is to a world.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Standing {
    Owner,
    Operator,
    Member,
}

/// What `account` is to `realm`: its owner, or one of its members.
pub fn standing(realm: &Realm, account: Uuid) -> Standing {
    if realm.owner.id == account {
        return Standing::Owner;
    }
    match realm
        .members
        .iter()
        .find(|member| member.id == account)
        .map(|member| member.role)
    {
        Some(MemberRole::Operator) => Standing::Operator,
        _ => Standing::Member,
    }
}

/// The word and colour of a state, on a card and in the pane; the colour is the card's dot.
pub fn state_look(state: RealmState) -> (&'static str, Color) {
    match state {
        RealmState::Stopped => ("Stopped", theme::muted()),
        RealmState::Starting => ("Starting...", theme::accent()),
        RealmState::Running => ("Running", theme::ok()),
        RealmState::Stopping => ("Stopping...", theme::muted()),
        RealmState::Failed => ("Failed", theme::danger()),
        RealmState::Archived => ("Archived", theme::muted()),
    }
}

/// Whether a world in `state` may be joined: not while it stops.
pub fn joinable(state: RealmState) -> bool {
    state != RealmState::Stopping
}

/// The line under the name of a world: whose it is, and what the player is to it.
pub fn owner_line(realm: &Realm, standing: Standing) -> String {
    match standing {
        Standing::Owner => "Your world".to_string(),
        Standing::Operator => format!("by {} \u{b7} operator", realm.owner.username),
        Standing::Member => format!("by {} \u{b7} member", realm.owner.username),
    }
}

/// Whether `me` may create another world, owning those of `realms` it owns.
pub fn can_create(me: &Me, realms: &[Realm]) -> bool {
    let owned = realms
        .iter()
        .filter(|realm| realm.owner.id == me.id)
        .count();
    me.roles.contains(&HubRole::Host) && owned < me.realm_quota as usize
}

/// The hint bar of the screen.
#[derive(Component, Default)]
struct HostedHints;

const REFRESH_HINT: Hint = Hint {
    keys: "F5",
    action: "Refresh",
};
const BACK_HINT: Hint = Hint {
    keys: "Esc",
    action: "Back",
};

impl ListHints for HostedHints {
    const CARDS: &'static [Hint] = &[
        Hint {
            keys: "↑↓",
            action: "Select",
        },
        Hint {
            keys: "Enter",
            action: "Join",
        },
        REFRESH_HINT,
        BACK_HINT,
    ];
    const SCREEN: &'static [Hint] = &[
        Hint {
            keys: "Tab",
            action: "Move",
        },
        Hint {
            keys: "Enter",
            action: "Select",
        },
        REFRESH_HINT,
        BACK_HINT,
    ];
}

/// The tab bar of the Multiplayer screen, `selected` checked: checking the other tab opens its
/// screen.
pub fn multiplayer_tabs(commands: &mut Commands, selected: usize, font: Handle<Font>) -> Entity {
    let tabs = Tabs {
        label: "Multiplayer",
        tabs: &[tr(SERVERS_TITLE), HOSTED_TITLE],
        selected,
    };
    let bar = spawn_tabs(commands, &tabs, font);
    commands.entity(bar).observe(
        |change: On<TabChange>, mut menu: ResMut<NextState<MenuState>>| {
            menu.set(if change.index == 0 {
                MenuState::Multi
            } else {
                MenuState::Hosted
            });
        },
    );
    bar
}

pub(super) fn plugin(app: &mut App) {
    hosted_dialogs::plugin(app);
    app.add_systems(OnEnter(MenuState::Hosted), hosted_setup)
        .add_systems(
            Update,
            (
                take_answers.run_if(resource_exists::<Account>),
                follow_the_account.run_if(resource_exists::<Account>),
                refresh_on_f5
                    .run_if(not(modal_open))
                    .run_if(not(text_field_focused))
                    .run_if(resource_exists::<Account>),
                follow_focus_with_hints::<HostedHints>,
            )
                .chain()
                .run_if(in_state(MenuState::Hosted)),
        )
        .add_observer(act_on_pane)
        .add_observer(join_on_enter);
}

/// Spawns the screen: the tabs and the buttons of the title bar, the body, Back and the hints,
/// then lists the worlds, the card of a session's world focused.
fn hosted_setup(
    end: Option<Res<SessionEnd>>,
    account: Option<Res<Account>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let account = account.as_deref().cloned().unwrap_or(Account::LoggedOut);
    let focus = end.as_deref().and_then(|end| end.origin.realm());
    commands.remove_resource::<SessionEnd>();
    let mut calls = HubCalls::default();
    if let Account::LoggedIn(file) = &account {
        list_worlds(&mut calls, file, focus);
    }
    let root = commands
        .spawn((
            menu_background(&assets),
            DespawnOnExit(MenuState::Hosted),
            TabGroup::new(0),
            HostedBoard {
                account: account_id(&account),
                ..default()
            },
            calls,
        ))
        .id();
    let content = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    left: px(theme::space_xl()),
                    right: px(theme::space_xl()),
                    top: px(theme::space_xl()),
                    bottom: px(HINT_BAR_HEIGHT + theme::space_xl()),
                },
                row_gap: px(theme::space_m()),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let title = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(theme::space_s()),
                ..default()
            },
            ChildOf(content),
        ))
        .id();
    let tabs = multiplayer_tabs(&mut commands, 1, font.clone());
    commands.entity(tabs).insert((HostedTabs, ChildOf(title)));
    // from the other tab, the focus stays on the tabs; back from a session, it goes to its card
    if focus.is_none() {
        commands.entity(tabs).insert(AutoFocus);
    }
    commands.spawn((
        Node {
            flex_grow: 1.0,
            ..default()
        },
        ChildOf(title),
    ));
    let actions = commands
        .spawn((
            Node {
                column_gap: px(theme::space_s()),
                ..default()
            },
            ChildOf(title),
        ))
        .id();
    let body = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_height: px(0),
                column_gap: px(theme::space_l()),
                ..default()
            },
            ChildOf(content),
        ))
        .id();
    commands.spawn((
        Node::default(),
        ChildOf(content),
        children![(
            button("Back", ButtonVariant::Default, font.clone()),
            observe(
                |_: On<Activate>, mut menu_state: ResMut<NextState<MenuState>>| {
                    menu_state.set(MenuState::Main);
                }
            ),
        )],
    ));
    commands.spawn((list_hint_bar::<HostedHints>(&assets), ChildOf(root)));
    commands.entity(root).insert(HostedParts { body, actions });
    show_logged_out_or_loading(&mut commands, body, &account, &font);
}

fn account_id(account: &Account) -> Option<Uuid> {
    match account {
        Account::LoggedIn(file) => Some(file.account),
        Account::LoggedOut => None,
    }
}

/// Asks the hub who the account is and which worlds it has, then selects the card of `focus`.
/// A listing still waiting for its answer is forgotten: it may be older than a change just made.
pub fn list_worlds(calls: &mut HubCalls, file: &AccountFile, focus: Option<Uuid>) {
    let (client, session) = (HubClient::new(&file.hub), file.session.clone());
    calls.0.retain(|call| !call.listing);
    calls.push(true, None, move || Answer::Listed {
        result: client
            .me(&session)
            .and_then(|me| Ok((me, client.realms(&session)?))),
        focus,
    });
}

/// Lists the worlds again on Refresh or F5, unless they are being listed.
fn refresh(account: &Account, calls: &mut Query<&mut HubCalls>, selected: Option<Uuid>) {
    if let (Account::LoggedIn(file), Ok(mut calls)) = (account, calls.single_mut()) {
        if !calls.listing() {
            list_worlds(&mut calls, file, selected);
        }
    }
}

/// The empty state of the body: `title`, and `line` under it in `color`.
fn show_notice(
    commands: &mut Commands,
    body: Entity,
    (title, line, color): (&str, &str, Color),
    font: &Handle<Font>,
) -> Entity {
    commands.entity(body).despawn_related::<Children>();
    let notice = commands
        .spawn((
            ListEmpty,
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(theme::space_m()),
                padding: UiRect::all(px(theme::space_xl())),
                border: UiRect::all(px(theme::bevel_width())),
                ..default()
            },
            BackgroundColor(theme::panel()),
            raised_bevel(),
            ChildOf(body),
            children![text(title, theme::font_l(), theme::text(), font)],
        ))
        .id();
    if !line.is_empty() {
        commands.spawn((text(line, theme::font_s(), color, font), ChildOf(notice)));
    }
    notice
}

/// What the body shows before the worlds come: the way to log in, or that they are asked for.
fn show_logged_out_or_loading(
    commands: &mut Commands,
    body: Entity,
    account: &Account,
    font: &Handle<Font>,
) {
    match account {
        Account::LoggedOut => {
            let notice = show_notice(commands, body, (LOGGED_OUT, "", theme::muted()), font);
            commands.spawn((
                button("Log in", ButtonVariant::Primary, font.clone()),
                ChildOf(notice),
                observe(|_: On<Activate>, mut next: ResMut<NextDialog>| {
                    next.0 = Some(Screen::Account);
                }),
            ));
        }
        Account::LoggedIn(_) => {
            show_notice(
                commands,
                body,
                ("Asking the hub...", "", theme::muted()),
                font,
            );
        }
    }
}

/// Lists the worlds again when the player logs in, out or into another account. The dialogs
/// open on the worlds of before close, a call they wait for with it.
fn follow_the_account(
    account: Res<Account>,
    mut boards: Query<(&mut HostedBoard, &mut HubCalls, &HostedParts)>,
    dialogs: Query<Entity, With<HostedDialog>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok((mut board, mut calls, screen)) = boards.single_mut() else {
        return;
    };
    let id = account_id(&account);
    if board.account == id {
        return;
    }
    // the worlds of the account of before are not this one's
    *board = HostedBoard {
        account: id,
        ..default()
    };
    calls.0.clear();
    for dialog in &dialogs {
        // without an action: a confirmation closed so must not stop or delete anything
        dismiss_dialog(&mut commands, dialog);
    }
    if let Account::LoggedIn(file) = &*account {
        list_worlds(&mut calls, file, None);
    }
    let font: Handle<Font> = assets.load(FONT_PATH);
    commands
        .entity(screen.actions)
        .despawn_related::<Children>();
    show_logged_out_or_loading(&mut commands, screen.body, &account, &font);
}

/// Asks for the worlds again on F5.
fn refresh_on_f5(
    keys: Res<ButtonInput<KeyCode>>,
    account: Res<Account>,
    mut calls: Query<&mut HubCalls>,
    selected: Query<&HostedCard, With<Selected>>,
) {
    if keys.just_pressed(KeyCode::F5) {
        refresh(
            &account,
            &mut calls,
            selected.single().ok().map(|card| card.0),
        );
    }
}

/// What the screen needs to show an answer.
type Screens<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut HostedBoard,
        &'static mut HubCalls,
        &'static HostedParts,
    ),
>;

/// Takes the answers of the hub: shows the worlds listed, closes the form of a world created,
/// and lists them again after a change, or says why a call failed. A session the hub refused
/// logs the account out, and `follow_the_account` shows the tab logged out.
#[allow(clippy::too_many_arguments)]
fn take_answers(
    mut screens: Screens,
    account: Res<Account>,
    selected: Query<&HostedCard, With<Selected>>,
    tabs: Query<(), With<HostedTabs>>,
    mut list_focus: ListFocus,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok((mut board, mut calls, parts)) = screens.single_mut() else {
        return;
    };
    let mut answers = Vec::new();
    let mut done = Vec::new();
    calls.0.retain(|call| {
        match call.answer.take() {
            Some(answer) => answers.push(answer),
            None if call.sent.elapsed() > CALL_DEADLINE => {
                warn!("A call to the hub never came back, forgetting it");
            }
            None => return true,
        }
        done.extend(call.dialog);
        false
    });
    // before the answers: what the dialog shows of them comes after
    for dialog in done {
        hosted_dialogs::members_idle(&mut commands, dialog);
    }
    if answers.is_empty() {
        return;
    }
    let font: Handle<Font> = assets.load(FONT_PATH);
    let Account::LoggedIn(file) = &*account else {
        return;
    };
    if answers.iter().any(Answer::unauthorized) {
        warn!("The hub no longer knows the session of the account, logging it out");
        end_revoked_session(&mut commands, file.account);
        return;
    }
    let hub = file.hub.clone();
    let say = |commands: &mut Commands, error: &HubError| {
        warn!("The hub refused or did not answer: {error}");
        show_toast(
            commands,
            &hub_message(error, &hub),
            ToastKind::Error,
            font.clone(),
        );
    };
    let reselect = selected.single().ok().map(|card| card.0);
    for answer in answers {
        match answer {
            Answer::Listed { result, focus } => match result {
                Ok((me, realms)) => {
                    board.me = Some(me);
                    board.realms = realms;
                    board.error = None;
                    board.listed = true;
                    // the focus stays on the tabs while the player goes from one to the other
                    let keep_focus = list_focus
                        .focus
                        .get()
                        .is_some_and(|focused| tabs.contains(focused));
                    show_board(
                        &mut commands,
                        (&board, parts, file.account),
                        focus.or(reselect),
                        (!keep_focus).then_some(&mut list_focus),
                        &font,
                    );
                    refill_members(&mut commands, &board.realms, file.account, &font);
                }
                Err(error) if board.listed => say(&mut commands, &error),
                Err(error) => {
                    warn!("Cannot list the hosted worlds: {error}");
                    board.error = Some(hub_message(&error, &hub));
                    show_board(
                        &mut commands,
                        (&board, parts, file.account),
                        None,
                        None,
                        &font,
                    );
                }
            },
            Answer::Created { result, form } => {
                hosted_dialogs::created(&mut commands, form, &result, &hub, &font);
                if let Ok(realm) = result {
                    list_worlds(&mut calls, file, Some(realm.id));
                }
            }
            Answer::Changed {
                result,
                done,
                focus,
                field,
            } => match result {
                Ok(()) => {
                    if !done.is_empty() {
                        show_toast(&mut commands, &done, ToastKind::Info, font.clone());
                    }
                    if let Some(field) = field {
                        hosted_dialogs::clear_field(&mut commands, field);
                    }
                    list_worlds(&mut calls, file, focus.or(reselect));
                }
                Err(error) => match field {
                    Some(field) => {
                        hosted_dialogs::field_error(&mut commands, field, &error, &hub);
                    }
                    None => say(&mut commands, &error),
                },
            },
            Answer::Lost => warn!("No thread could call the hub"),
        }
    }
}

/// Shows the board in the body: the cards and the pane of the selected world (`focus`, else the
/// first), or why there is none. The buttons of the title bar follow what the account may do.
/// The card is focused when `list_focus` is given.
fn show_board(
    commands: &mut Commands,
    (board, parts, account): (&HostedBoard, &HostedParts, Uuid),
    focus: Option<Uuid>,
    mut list_focus: Option<&mut ListFocus>,
    font: &Handle<Font>,
) {
    commands.entity(parts.actions).despawn_related::<Children>();
    commands.spawn((
        button("Refresh", ButtonVariant::Default, font.clone()),
        RefreshButton,
        ChildOf(parts.actions),
        observe(
            |_: On<Activate>,
             account: Res<Account>,
             mut calls: Query<&mut HubCalls>,
             selected: Query<&HostedCard, With<Selected>>| {
                refresh(
                    &account,
                    &mut calls,
                    selected.single().ok().map(|card| card.0),
                );
            },
        ),
    ));
    let creatable = board
        .me
        .as_ref()
        .is_some_and(|me| can_create(me, &board.realms));
    if creatable {
        commands.spawn((
            button(CREATE_WORLD, ButtonVariant::Default, font.clone()),
            ChildOf(parts.actions),
            observe(
                |_: On<Activate>,
                 assets: Res<AssetServer>,
                 focus: Res<InputFocus>,
                 mut commands: Commands| {
                    open_create_dialog(&mut commands, &assets, &focus);
                },
            ),
        ));
    }
    if let Some(error) = &board.error {
        show_notice(
            commands,
            parts.body,
            ("Cannot list your hosted worlds", error, theme::danger()),
            font,
        );
        return;
    }
    if board.realms.is_empty() {
        let line = if creatable {
            "Create one with Create a world."
        } else {
            "When a friend adds you to a world, it shows here."
        };
        show_notice(
            commands,
            parts.body,
            ("No hosted worlds yet", line, theme::muted()),
            font,
        );
        return;
    }
    commands.entity(parts.body).despawn_related::<Children>();
    let scroll = spawn_scroll_list(commands, WheelStep::Item);
    commands.entity(scroll.frame).insert(ChildOf(parts.body));
    commands
        .entity(scroll.list)
        .insert((
            HostedList,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::ListBox)),
            AccessibleLabel::new(HOSTED_TITLE),
        ))
        .observe(show_selected_world);
    let pane = commands
        .spawn((
            HostedPane,
            Node {
                width: px(DETAIL_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(px(theme::space_l())),
                border: UiRect::all(px(theme::bevel_width())),
                row_gap: px(theme::space_l()),
                ..default()
            },
            BackgroundColor(theme::panel()),
            raised_bevel(),
            ChildOf(parts.body),
        ))
        .id();
    let chosen = focus
        .and_then(|id| board.realms.iter().position(|realm| realm.id == id))
        .unwrap_or(0);
    for (index, realm) in board.realms.iter().enumerate() {
        let card = spawn_card(commands, scroll.list, realm, account, font);
        if index == chosen {
            // the list sends no `CardSelected` for a card selected here: the pane is filled now
            commands.entity(card).insert((Selected, TabIndex(0)));
            if let Some(list_focus) = list_focus.as_deref_mut() {
                list_focus.set(card, true);
            }
            show_pane(commands, pane, realm, account, font);
        }
    }
}

/// Spawns the card of `realm` at the end of `list`, and returns it.
fn spawn_card(
    commands: &mut Commands,
    list: Entity,
    realm: &Realm,
    account: Uuid,
    font: &Handle<Font>,
) -> Entity {
    let card = spawn_list_card(commands, list, &realm.name);
    let (word, color) = state_look(realm.state);
    // each text in a box of its own, as on the Servers tab
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
        HostedCard(realm.id),
        children![
            (
                Node {
                    width: px(DOT_SIDE),
                    height: px(DOT_SIDE),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(color),
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    min_width: px(0),
                    row_gap: px(theme::space_xs()),
                    ..default()
                },
                children![
                    boxed(text(&realm.name, theme::font_m(), theme::text(), font)),
                    boxed(text(
                        &owner_line(realm, standing(realm, account)),
                        theme::font_s(),
                        theme::muted(),
                        font
                    )),
                ],
            ),
            (
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
                children![text(word, theme::font_s(), color, font)],
            ),
        ],
    ));
    card
}

/// Shows `realm` in the pane, in place of what it showed: its facts, and the buttons the
/// player's role allows.
fn show_pane(
    commands: &mut Commands,
    pane: Entity,
    realm: &Realm,
    account: Uuid,
    font: &Handle<Font>,
) {
    commands.entity(pane).despawn_related::<Children>();
    let standing = standing(realm, account);
    let (word, color) = state_look(realm.state);
    let facts = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::space_xs()),
                min_width: px(0),
                ..default()
            },
            ChildOf(pane),
        ))
        .id();
    commands.spawn((
        text(&realm.name, theme::font_l(), theme::text(), font),
        ChildOf(facts),
    ));
    commands.spawn((
        text(
            &owner_line(realm, standing),
            theme::font_s(),
            theme::muted(),
            font,
        ),
        ChildOf(facts),
    ));
    commands.spawn((
        text(word, theme::font_m(), color, font),
        Node {
            margin: UiRect::top(px(theme::space_s())),
            ..default()
        },
        ChildOf(facts),
    ));
    if let Some(address) = &realm.address {
        commands.spawn((
            text(
                &typed_address(address),
                theme::font_s(),
                theme::muted(),
                font,
            ),
            ChildOf(facts),
        ));
    }
    let buttons = commands
        .spawn((
            ArrowNav,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::space_s()),
                ..default()
            },
            ChildOf(pane),
        ))
        .id();
    let id = realm.id;
    let mut join = commands.spawn((
        button("Join", ButtonVariant::Primary, font.clone()),
        PaneAction(id, PaneKind::Join),
        ChildOf(buttons),
    ));
    if !joinable(realm.state) {
        join.insert(InteractionDisabled);
    }
    if standing != Standing::Member {
        commands.spawn((
            button("Members", ButtonVariant::Default, font.clone()),
            PaneAction(id, PaneKind::Members),
            ChildOf(buttons),
        ));
    }
    if standing != Standing::Owner {
        return;
    }
    let pair = commands
        .spawn((
            Node {
                column_gap: px(theme::space_s()),
                ..default()
            },
            ChildOf(buttons),
        ))
        .id();
    let stoppable = matches!(realm.state, RealmState::Starting | RealmState::Running);
    for (label, kind) in [("Stop", PaneKind::Stop), ("Delete", PaneKind::Delete)] {
        let mut entity = commands.spawn((
            button(label, ButtonVariant::Default, font.clone()),
            PaneAction(id, kind),
            ChildOf(pair),
        ));
        if kind == PaneKind::Stop && !stoppable {
            entity.insert(InteractionDisabled);
        }
        entity.entry::<Node>().and_modify(|mut node| {
            // halves of the width, whatever their labels
            node.flex_grow = 1.0;
            node.flex_basis = px(0);
        });
    }
}

/// Shows the world of the card just selected in the pane.
fn show_selected_world(
    selected: On<CardSelected>,
    cards: Query<&HostedCard>,
    boards: Query<&HostedBoard>,
    panes: Query<Entity, With<HostedPane>>,
    account: Res<Account>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let (Ok(card), Ok(board), Ok(pane), Account::LoggedIn(file)) = (
        cards.get(selected.card),
        boards.single(),
        panes.single(),
        &*account,
    ) else {
        return;
    };
    if let Some(realm) = board.realms.iter().find(|realm| realm.id == card.0) {
        show_pane(
            &mut commands,
            pane,
            realm,
            file.account,
            &assets.load(FONT_PATH),
        );
    }
}

/// What joining a world needs from the app.
#[derive(bevy::ecs::system::SystemParam)]
struct Joining<'w> {
    account: Res<'w, Account>,
    probe: Res<'w, StatusProbe>,
    resolver: Res<'w, AddressResolver>,
    target: ResMut<'w, TargetServer>,
    game: ResMut<'w, NextState<GameState>>,
    menu: ResMut<'w, NextState<MenuState>>,
}

impl Joining<'_> {
    /// Joins `realm`, which the hub starts first if it is stopped; not while it stops.
    fn join(&mut self, realm: &Realm, commands: &mut Commands) {
        let Account::LoggedIn(file) = &*self.account else {
            return;
        };
        if !joinable(realm.state) {
            return;
        }
        let join = HostedJoin {
            realm: realm.id,
            name: realm.name.clone(),
        };
        join_hosted_world(
            &join,
            file,
            (*self.probe, *self.resolver),
            &mut self.target,
            commands,
            (&mut self.game, &mut self.menu),
        );
    }
}

/// Carries out the button of the pane that was activated.
fn act_on_pane(
    activate: On<Activate>,
    actions: Query<&PaneAction>,
    boards: Query<&HostedBoard>,
    mut joining: Joining,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    let (Ok(&PaneAction(id, kind)), Ok(board)) = (actions.get(activate.entity), boards.single())
    else {
        return;
    };
    let (Some(realm), Account::LoggedIn(file)) = (
        board.realms.iter().find(|realm| realm.id == id),
        &*joining.account,
    ) else {
        return;
    };
    match kind {
        PaneKind::Join => {
            let realm = realm.clone();
            joining.join(&realm, &mut commands);
        }
        PaneKind::Members => {
            let editable = standing(realm, file.account) == Standing::Owner;
            open_members_dialog(&mut commands, &assets, &focus, realm, editable);
        }
        PaneKind::Stop => open_stop_dialog(&mut commands, &assets, &focus, realm),
        PaneKind::Delete => {
            let next = neighbour_of(&board.realms, id);
            open_delete_dialog(&mut commands, &assets, &focus, realm, next);
        }
    }
}

/// The world whose card takes the focus once `id` is gone: the next one, else the previous.
fn neighbour_of(realms: &[Realm], id: Uuid) -> Option<Uuid> {
    let index = realms.iter().position(|realm| realm.id == id)?;
    realms
        .get(index + 1)
        .or_else(|| {
            index
                .checked_sub(1)
                .and_then(|previous| realms.get(previous))
        })
        .map(|realm| realm.id)
}

/// Enter on a focused card joins its world, unless Control is held.
fn join_on_enter(
    mut event: On<FocusedInput<KeyboardInput>>,
    cards: Query<&HostedCard>,
    boards: Query<&HostedBoard>,
    mut joining: Joining,
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    // the event bubbles: only the step on the card itself is handled
    let Ok(card) = cards.get(event.focused_entity) else {
        return;
    };
    let input = &event.input;
    if input.state != ButtonState::Pressed
        || input.repeat
        || input.key_code != KeyCode::Enter
        || keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
    {
        return;
    }
    event.propagate(false);
    let Ok(board) = boards.single() else {
        return;
    };
    if let Some(realm) = board
        .realms
        .iter()
        .find(|realm| realm.id == card.0)
        .cloned()
    {
        joining.join(&realm, &mut commands);
    }
}

#[cfg(test)]
mod tests {
    use rustcraft_protocol::api::{AccountSummary, RealmMember};

    use super::*;

    fn summary(id: u128, username: &str) -> AccountSummary {
        AccountSummary {
            id: Uuid::from_u128(id),
            username: username.into(),
        }
    }

    fn realm(owner: u128, members: &[(u128, MemberRole)]) -> Realm {
        Realm {
            id: Uuid::from_u128(100 + owner),
            name: "Home".into(),
            owner: summary(owner, "Alex"),
            state: RealmState::Stopped,
            address: None,
            members: members
                .iter()
                .map(|&(id, role)| RealmMember {
                    id: Uuid::from_u128(id),
                    username: "Steve".into(),
                    role,
                })
                .collect(),
        }
    }

    #[test]
    fn the_player_is_the_owner_an_operator_or_a_member() {
        let realm = realm(1, &[(2, MemberRole::Operator), (3, MemberRole::Member)]);
        assert_eq!(standing(&realm, Uuid::from_u128(1)), Standing::Owner);
        assert_eq!(standing(&realm, Uuid::from_u128(2)), Standing::Operator);
        assert_eq!(standing(&realm, Uuid::from_u128(3)), Standing::Member);
        assert_eq!(owner_line(&realm, Standing::Owner), "Your world");
        assert_eq!(
            owner_line(&realm, Standing::Operator),
            "by Alex \u{b7} operator"
        );
    }

    #[test]
    fn creating_needs_the_host_role_and_room_in_the_quota() {
        let me = |roles: Vec<HubRole>, realm_quota| Me {
            id: Uuid::from_u128(1),
            username: "Alex".into(),
            roles,
            realm_quota,
            created_at: 0,
        };
        let owned = [realm(1, &[])];
        let joined = [realm(2, &[(1, MemberRole::Member)])];
        assert!(can_create(&me(vec![HubRole::Host], 1), &joined));
        assert!(!can_create(&me(vec![HubRole::Host], 1), &owned));
        assert!(can_create(&me(vec![HubRole::Host], 2), &owned));
        assert!(!can_create(&me(vec![], 2), &[]));
    }

    #[test]
    fn a_deleted_card_hands_the_focus_to_the_next_else_the_previous() {
        let realms = [realm(1, &[]), realm(2, &[]), realm(3, &[])];
        let id = |owner: u128| Uuid::from_u128(100 + owner);
        assert_eq!(neighbour_of(&realms, id(1)), Some(id(2)));
        assert_eq!(neighbour_of(&realms, id(3)), Some(id(2)));
        assert_eq!(neighbour_of(&realms[..1], id(1)), None);
    }

    #[test]
    fn a_stopping_world_cannot_be_joined() {
        assert!(!joinable(RealmState::Stopping));
        for state in [RealmState::Stopped, RealmState::Running, RealmState::Failed] {
            assert!(joinable(state), "{state:?}");
        }
    }

    #[test]
    fn each_state_has_its_word_and_colour() {
        assert_eq!(state_look(RealmState::Running), ("Running", theme::ok()));
        assert_eq!(state_look(RealmState::Failed).1, theme::danger());
        assert_eq!(state_look(RealmState::Stopped).1, theme::muted());
    }
}
