//! The dialogs of the Hosted worlds tab: the form of a new world, the members of a world (which
//! its owner changes, and its operators read), and the confirmations of Stop and Delete. What
//! they ask of the hub runs on a thread (`HubCalls`), and the tab shows the answer.

use crate::i18n::tr;
use bevy::{
    input_focus::{FocusCause, InputFocus},
    prelude::*,
    text::{EditableText, EditableTextFilter},
    ui::InteractionDisabled,
    ui_widgets::Activate,
};
use rustcraft_protocol::api::{CreateRealmRequest, MemberRole, Realm, RealmSettings};
use shared::players::game_mode::GameMode;
use shared::world::{hostiles::Difficulty, WorldPreset};
use uuid::Uuid;

use super::super::solo::dialogs::{spawn_world_choice, DIFFICULTIES, GAME_MODES, WORLD_TYPES};
use super::super::text;
use super::super::worlds::{parse_seed, EMPTY_WORLD_NAME};
use super::hosted::{Answer, HubCalls};
use crate::account::hub::{hub_message, HubClient, HubError};
use crate::account::Account;
use crate::ui::assets::FONT_PATH;
use crate::ui::menus::account::add_to_panel;
use crate::ui::theme::{FONT_M, FONT_S, MUTED, SPACE_S, TEXT};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::dialog::{
    apply_form_check, close_dialog, set_dialog_width, spawn_dialog, spawn_form_dialog, Dialog,
    DialogClosed, DialogSubmit, FormChanged,
};
use crate::ui::widgets::hints::Hint;
use crate::ui::widgets::text_field::{set_text, spawn_text_field, value_of, FieldError, TextField};
use crate::ui::widgets::toast::{show_toast, ToastKind};

/// The title of the form of a new world.
pub const CREATE_TITLE: &str = "Create a world";
/// The longest name of a world, in characters, as the hub takes it.
const NAME_LENGTH: usize = 32;
/// The most digits a seed has: the hub takes a `u32`.
const SEED_DIGITS: usize = 10;
/// The longest username of the hub.
const USERNAME_LENGTH: usize = 20;
/// Shown under the name field of a name the hub would refuse.
const INVALID_NAME: &str = "1 to 32 characters, no control character";

const FORM_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Create",
    },
    Hint {
        keys: "Esc",
        action: "Cancel",
    },
];

const MEMBERS_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Add",
    },
    Hint {
        keys: "Esc",
        action: "Close",
    },
];

/// A dialog of the tab, on its root: it closes when the account changes, as it shows a world of
/// the account of before.
#[derive(Component)]
pub struct HostedDialog;

/// The form of a new world, on the root of its dialog.
#[derive(Component)]
struct CreateForm {
    name: Entity,
    seed: Entity,
    /// Whether the hub is creating the world.
    busy: bool,
}

/// What the form of a new world has checked in its choices.
#[derive(Component)]
struct HostedChoices {
    game_mode: GameMode,
    difficulty: Difficulty,
    preset: WorldPreset,
}

impl Default for HostedChoices {
    fn default() -> Self {
        Self {
            game_mode: GAME_MODES[0].1,
            difficulty: Difficulty::default(),
            preset: WORLD_TYPES[0].1.clone(),
        }
    }
}

/// The settings of a new world, as the server's flags take their names: `--world-type` in
/// kebab-case.
fn settings_of(choices: &HostedChoices, seed: Option<u32>) -> RealmSettings {
    RealmSettings {
        seed: seed.map(u64::from),
        world_type: Some(choices.preset.name().replace('_', "-")),
        game_mode: Some(choices.game_mode.name().to_string()),
        difficulty: Some(choices.difficulty.name().to_string()),
    }
}

/// The open Members dialog of the world `realm`: where its rows are, and its field to add a
/// player, for the owner.
#[derive(Component)]
pub struct MembersDialog {
    realm: Uuid,
    rows: Entity,
    input: Option<Entity>,
    /// Whether the hub is changing a member: Add and the buttons of the rows wait for it.
    busy: bool,
}

/// A button of a row of the Members dialog: what it changes of the member `username`.
#[derive(Component, Clone)]
pub struct MemberAction {
    realm: Uuid,
    pub username: String,
    change: MemberChange,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MemberChange {
    Role(MemberRole),
    Remove,
}

pub(super) fn plugin(app: &mut App) {
    app.add_observer(change_member);
}

/// Spawns a call to the hub with the account's client and session, on the tab's calls.
fn call_hub(
    account: &Account,
    calls: &mut Query<&mut HubCalls>,
    call: impl FnOnce(&HubClient, &str) -> Answer + Send + 'static,
) {
    let (Account::LoggedIn(file), Ok(mut calls)) = (account, calls.single_mut()) else {
        return;
    };
    let (client, session) = (HubClient::new(&file.hub), file.session.clone());
    calls.spawn(move || call(&client, &session));
}

/// Opens the form of a new world: its name, an optional seed, and the choices of the world.
pub fn open_create_dialog(commands: &mut Commands, assets: &AssetServer, focus: &InputFocus) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let mut field = |label, placeholder, max_characters| {
        spawn_text_field(
            commands,
            &TextField {
                label,
                placeholder,
                max_characters: Some(max_characters),
                font: font.clone(),
                font_size: FONT_M,
                reserve_error: true,
            },
        )
    };
    let name = field("Name", "", NAME_LENGTH);
    let seed = field("Seed (optional)", "Random", SEED_DIGITS);
    commands
        .entity(seed.input)
        .insert(EditableTextFilter::new(|c: char| c.is_ascii_digit()));
    let dialog = Dialog {
        title: CREATE_TITLE,
        body: "The hub hosts it: your friends join it even when you are away.",
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Create", ButtonVariant::Primary),
        ],
        focused: 0,
    };
    let parts = spawn_form_dialog(
        commands,
        &dialog,
        &[("Name", name), ("Seed (optional)", seed)],
        &FORM_HINTS,
        font.clone(),
        focus,
    );
    let root = parts.root;
    if let Some(form) = parts.form {
        spawn_world_choice(
            commands,
            (root, form),
            (
                tr("worlds.game_mode"),
                &GAME_MODES.map(|(caption, _)| tr(caption)),
                0,
            ),
            &font,
            |choices: &mut HostedChoices, index| choices.game_mode = GAME_MODES[index].1,
        );
        let normal = DIFFICULTIES
            .iter()
            .position(|&(_, difficulty)| difficulty == Difficulty::default())
            .unwrap_or_default();
        spawn_world_choice(
            commands,
            (root, form),
            (
                tr("worlds.difficulty"),
                &DIFFICULTIES.map(|(caption, _)| tr(caption)),
                normal,
            ),
            &font,
            |choices: &mut HostedChoices, index| choices.difficulty = DIFFICULTIES[index].1,
        );
        spawn_world_choice(
            commands,
            (root, form),
            (
                tr("worlds.world_type"),
                &WORLD_TYPES.map(|(caption, _)| tr(caption)),
                0,
            ),
            &font,
            |choices: &mut HostedChoices, index| {
                choices.preset = WORLD_TYPES[index].1.clone();
            },
        );
    }
    commands
        .entity(root)
        .insert((
            HostedDialog,
            CreateForm {
                name: name.input,
                seed: seed.input,
                busy: false,
            },
            HostedChoices::default(),
        ))
        .observe(check_create_form)
        .observe(create_on_submit);
}

/// What is wrong with the name and the seed typed, and whether the form can be sent. The hub is
/// the authority: this is for an instant answer.
fn check_create(name: &str, seed: &str) -> ([Option<&'static str>; 2], bool) {
    let name = name.trim();
    let name_error = if name.is_empty() {
        Some(EMPTY_WORLD_NAME)
    } else if name.chars().count() > NAME_LENGTH || name.chars().any(char::is_control) {
        Some(INVALID_NAME)
    } else {
        None
    };
    let seed_error = parse_seed(seed).err();
    let errors = [name_error, seed_error];
    (errors, errors.iter().all(Option::is_none))
}

fn check_create_form(changed: On<FormChanged>, forms: Query<&CreateForm>, mut commands: Commands) {
    let Ok(form) = forms.get(changed.entity) else {
        return;
    };
    let [name, seed] = [0, 1].map(|i| changed.values.get(i).cloned().unwrap_or_default());
    let (errors, submittable) = check_create(&name, &seed);
    let shown = errors
        .into_iter()
        .zip(&changed.touched)
        .map(|(error, &touched)| error.filter(|_| touched))
        .collect();
    apply_form_check(
        &mut commands,
        changed.entity,
        shown,
        submittable && !form.busy,
    );
}

/// Asks the hub to create the world of the form.
fn create_on_submit(
    submit: On<DialogSubmit>,
    mut forms: Query<(&mut CreateForm, &HostedChoices)>,
    texts: Query<&EditableText>,
    account: Res<Account>,
    mut calls: Query<&mut HubCalls>,
    mut commands: Commands,
) {
    let form_root = submit.entity;
    let Ok((mut form, choices)) = forms.get_mut(form_root) else {
        return;
    };
    let (name, seed) = (value_of(&texts, form.name), value_of(&texts, form.seed));
    if form.busy || !check_create(&name, &seed).1 {
        return;
    }
    let request = CreateRealmRequest {
        name: name.trim().to_string(),
        settings: settings_of(choices, parse_seed(&seed).ok().flatten()),
    };
    form.busy = true;
    apply_form_check(&mut commands, form_root, vec![None, None], false);
    call_hub(&account, &mut calls, move |client, session| {
        Answer::Created {
            result: client.create_realm(session, &request),
            form: form_root,
        }
    });
}

/// Closes the form `form` of a world the hub created, or makes it usable again and says under
/// the name why the hub did not.
pub(super) fn created(
    commands: &mut Commands,
    form: Entity,
    result: &Result<Realm, HubError>,
    hub: &str,
    font: &Handle<Font>,
) {
    match result {
        Ok(realm) => {
            info!("Created the hosted world {:?}", realm.name);
            commands.queue(move |world: &mut World| {
                if world.get::<CreateForm>(form).is_some() {
                    close_dialog(&mut world.commands(), form, true);
                    world.flush();
                }
            });
            show_toast(
                commands,
                &format!("Created \"{}\".", realm.name),
                ToastKind::Info,
                font.clone(),
            );
        }
        Err(error) => {
            warn!("The hub did not create the world: {error}");
            let message = hub_message(error, hub);
            commands.queue(move |world: &mut World| {
                let Some(mut open) = world.get_mut::<CreateForm>(form) else {
                    return;
                };
                open.busy = false;
                let name = open.name;
                apply_form_check(&mut world.commands(), form, vec![None, None], true);
                world.flush();
                // under the name: the hub refuses a name, or one world more than the quota
                world.entity_mut(name).insert(FieldError(Some(message)));
            });
        }
    }
}

/// Opens the members of `realm`: its owner and its members with their role. The owner
/// (`editable`) makes a member an operator or a member again, removes one and adds a player by
/// their username; anyone else only reads.
pub fn open_members_dialog(
    commands: &mut Commands,
    assets: &AssetServer,
    focus: &InputFocus,
    realm: &Realm,
    editable: bool,
) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let title = format!("Members of \"{}\"", realm.name);
    let rows = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_S),
            ..default()
        })
        .id();
    let (root, input) = if editable {
        let field = spawn_text_field(
            commands,
            &TextField {
                label: "Add a player",
                placeholder: "Username",
                max_characters: Some(USERNAME_LENGTH),
                font: font.clone(),
                font_size: FONT_M,
                reserve_error: true,
            },
        );
        let dialog = Dialog {
            title: &title,
            body: "",
            actions: &[
                ("Close", ButtonVariant::Default),
                ("Add", ButtonVariant::Primary),
            ],
            focused: 0,
        };
        let parts = spawn_form_dialog(
            commands,
            &dialog,
            &[("Add a player", field)],
            &MEMBERS_HINTS,
            font.clone(),
            focus,
        );
        if let Some(form) = parts.form {
            commands.entity(form).insert_children(0, &[rows]);
        }
        commands
            .entity(parts.root)
            .observe(check_member_form)
            .observe(add_member_on_submit);
        (parts.root, Some(field.input))
    } else {
        let dialog = Dialog {
            title: &title,
            body: "Only the owner changes who is a member.",
            actions: &[("Close", ButtonVariant::Primary)],
            focused: 0,
        };
        let root = spawn_dialog(commands, &dialog, font.clone(), focus);
        add_to_panel(commands, root, vec![rows]);
        (root, None)
    };
    set_dialog_width(commands, root, 560.0);
    commands.entity(root).insert((
        HostedDialog,
        MembersDialog {
            realm: realm.id,
            rows,
            input,
            busy: false,
        },
    ));
    fill_rows(commands, rows, realm, (editable, false), &font);
}

/// Shows the owner and the members of `realm` in `rows`, in place of what it showed; with the
/// buttons that change them when `editable`, disabled while `busy`.
fn fill_rows(
    commands: &mut Commands,
    rows: Entity,
    realm: &Realm,
    (editable, busy): (bool, bool),
    font: &Handle<Font>,
) {
    commands.entity(rows).despawn_related::<Children>();
    let row = |commands: &mut Commands, name: &str, role: &str| {
        commands
            .spawn((
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(SPACE_S),
                    ..default()
                },
                ChildOf(rows),
                children![
                    (
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0),
                            ..default()
                        },
                        children![text(name, FONT_M, TEXT, font)],
                    ),
                    text(role, FONT_S, MUTED, font),
                ],
            ))
            .id()
    };
    row(commands, &realm.owner.username, "Owner");
    if realm.members.is_empty() {
        commands.spawn((text("No members yet.", FONT_S, MUTED, font), ChildOf(rows)));
    }
    for member in &realm.members {
        let (role, other, change_label) = match member.role {
            MemberRole::Operator => ("Operator", MemberRole::Member, "Make member"),
            MemberRole::Member => ("Member", MemberRole::Operator, "Make operator"),
        };
        let line = row(commands, &member.username, role);
        if !editable {
            continue;
        }
        for (label, change) in [
            (change_label, MemberChange::Role(other)),
            ("Remove", MemberChange::Remove),
        ] {
            let mut action = commands.spawn((
                button(label, ButtonVariant::Default, font.clone()),
                MemberAction {
                    realm: realm.id,
                    username: member.username.clone(),
                    change,
                },
                ChildOf(line),
            ));
            if busy {
                action.insert(InteractionDisabled);
            }
        }
    }
}

/// Shows the members of `realms` again in the Members dialogs open on them, once they were
/// listed again. A button that goes with its row leaves the focus on the field.
pub(super) fn refill_members(
    commands: &mut Commands,
    realms: &[Realm],
    account: Uuid,
    font: &Handle<Font>,
) {
    let (realms, font) = (realms.to_vec(), font.clone());
    commands.queue(move |world: &mut World| {
        let mut dialogs = world.query::<&MembersDialog>();
        let open: Vec<(Uuid, Entity, bool)> = dialogs
            .iter(world)
            .map(|dialog| (dialog.realm, dialog.rows, dialog.busy))
            .collect();
        let focused = world.resource::<InputFocus>().get();
        let on_a_row = focused.is_some_and(|entity| world.get::<MemberAction>(entity).is_some());
        let mut commands = world.commands();
        for (id, rows, busy) in open {
            let Some(realm) = realms.iter().find(|realm| realm.id == id) else {
                continue;
            };
            let editable = realm.owner.id == account;
            fill_rows(&mut commands, rows, realm, (editable, busy), &font);
        }
        world.flush();
        if on_a_row {
            if let Some(input) = world
                .query::<&MembersDialog>()
                .iter(world)
                .find_map(|dialog| dialog.input)
            {
                world
                    .resource_mut::<InputFocus>()
                    .set(input, FocusCause::Navigated);
            }
        }
    });
}

fn check_member_form(
    changed: On<FormChanged>,
    dialogs: Query<&MembersDialog>,
    mut commands: Commands,
) {
    let typed = changed
        .values
        .first()
        .is_some_and(|name| !name.trim().is_empty());
    let busy = dialogs.get(changed.entity).is_ok_and(|dialog| dialog.busy);
    apply_form_check(&mut commands, changed.entity, vec![None], typed && !busy);
}

/// Makes the Members dialog `dialog` wait for the hub (`busy`), or usable again: Add, when a
/// name is typed, and the buttons of its rows.
fn set_members_busy(world: &mut World, dialog: Entity, busy: bool) {
    let Some(mut open) = world.get_mut::<MembersDialog>(dialog) else {
        return;
    };
    open.busy = busy;
    let (realm, input) = (open.realm, open.input);
    let actions: Vec<Entity> = world
        .query::<(Entity, &MemberAction)>()
        .iter(world)
        .filter(|(_, action)| action.realm == realm)
        .map(|(entity, _)| entity)
        .collect();
    for action in actions {
        if busy {
            world.entity_mut(action).insert(InteractionDisabled);
        } else {
            world.entity_mut(action).remove::<InteractionDisabled>();
        }
    }
    if let Some(input) = input {
        let typed = world
            .get::<EditableText>(input)
            .is_some_and(|text| !text.value().to_string().trim().is_empty());
        apply_form_check(&mut world.commands(), dialog, vec![None], typed && !busy);
        world.flush();
    }
}

/// Makes the Members dialog `dialog` usable again, once the hub answered its change.
pub(super) fn members_idle(commands: &mut Commands, dialog: Entity) {
    commands.queue(move |world: &mut World| set_members_busy(world, dialog, false));
}

/// Adds the player typed in the field as a member.
fn add_member_on_submit(
    submit: On<DialogSubmit>,
    dialogs: Query<&MembersDialog>,
    texts: Query<&EditableText>,
    account: Res<Account>,
    mut calls: Query<&mut HubCalls>,
    mut commands: Commands,
) {
    let root = submit.entity;
    let Ok(dialog) = dialogs.get(root) else {
        return;
    };
    let (Some(input), false) = (dialog.input, dialog.busy) else {
        return;
    };
    let username = value_of(&texts, input).trim().to_string();
    if username.is_empty() {
        return;
    }
    let realm = dialog.realm;
    commands.queue(move |world: &mut World| set_members_busy(world, root, true));
    call_hub(&account, &mut calls, move |client, session| {
        Answer::Changed {
            result: client.set_member(session, realm, &username, MemberRole::Member),
            done: format!("Added {username}."),
            focus: Some(realm),
            field: Some(input),
            dialog: Some(root),
        }
    });
}

/// Carries out a button of a row of the Members dialog.
fn change_member(
    activate: On<Activate>,
    actions: Query<&MemberAction, Without<InteractionDisabled>>,
    dialogs: Query<(Entity, &MembersDialog)>,
    account: Res<Account>,
    mut calls: Query<&mut HubCalls>,
    mut commands: Commands,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    let MemberAction {
        realm,
        username,
        change,
    } = action.clone();
    let Some((root, _)) = dialogs
        .iter()
        .find(|(_, dialog)| dialog.realm == realm && !dialog.busy)
    else {
        return;
    };
    commands.queue(move |world: &mut World| set_members_busy(world, root, true));
    call_hub(&account, &mut calls, move |client, session| {
        let (result, done) = match change {
            MemberChange::Role(role) => (
                client.set_member(session, realm, &username, role),
                match role {
                    MemberRole::Operator => format!("{username} is now an operator."),
                    MemberRole::Member => format!("{username} is now a member."),
                },
            ),
            MemberChange::Remove => (
                client.remove_member(session, realm, &username),
                format!("Removed {username}."),
            ),
        };
        Answer::Changed {
            result,
            done,
            focus: Some(realm),
            field: None,
            dialog: Some(root),
        }
    });
}

/// Empties the field of a player just added.
pub(super) fn clear_field(commands: &mut Commands, field: Entity) {
    commands.queue(move |world: &mut World| {
        if let Some(mut text) = world.get_mut::<EditableText>(field) {
            set_text(&mut text, "");
        }
        if let Ok(mut entity) = world.get_entity_mut(field) {
            entity.insert(FieldError(None));
        }
    });
}

/// Shows under `field` why the hub refused what was typed in it.
pub(super) fn field_error(commands: &mut Commands, field: Entity, error: &HubError, hub: &str) {
    let message = hub_message(error, hub);
    commands.queue(move |world: &mut World| {
        if let Ok(mut entity) = world.get_entity_mut(field) {
            entity.insert(FieldError(Some(message)));
        }
    });
}

/// Asks the owner whether to stop `realm`, then asks the hub.
pub fn open_stop_dialog(
    commands: &mut Commands,
    assets: &AssetServer,
    focus: &InputFocus,
    realm: &Realm,
) {
    let title = format!("Stop \"{}\"?", realm.name);
    let dialog = Dialog {
        title: &title,
        body: "Its players go back to their menus. The world is saved, and starts again when \
               someone joins it.",
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Stop", ButtonVariant::Danger),
        ],
        focused: 0,
    };
    let root = spawn_dialog(commands, &dialog, assets.load(FONT_PATH), focus);
    set_dialog_width(commands, root, 480.0);
    commands.entity(root).insert(HostedDialog);
    let id = realm.id;
    commands.entity(root).observe(
        move |closed: On<DialogClosed>, account: Res<Account>, mut calls: Query<&mut HubCalls>| {
            if closed.action != Some(1) {
                return;
            }
            call_hub(&account, &mut calls, move |client, session| {
                Answer::Changed {
                    result: client.stop_realm(session, id),
                    done: String::new(),
                    focus: Some(id),
                    field: None,
                    dialog: None,
                }
            });
        },
    );
}

/// Asks the owner whether to delete `realm`, then asks the hub; the card of `next` takes the
/// focus.
pub fn open_delete_dialog(
    commands: &mut Commands,
    assets: &AssetServer,
    focus: &InputFocus,
    realm: &Realm,
    next: Option<Uuid>,
) {
    let title = format!("Delete \"{}\"?", realm.name);
    let dialog = Dialog {
        title: &title,
        body: "It leaves the list of each of its members. Its files are kept for 30 days, then \
               removed.",
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Delete", ButtonVariant::Danger),
        ],
        focused: 0,
    };
    let root = spawn_dialog(commands, &dialog, assets.load(FONT_PATH), focus);
    set_dialog_width(commands, root, 480.0);
    commands.entity(root).insert(HostedDialog);
    let (id, name) = (realm.id, realm.name.clone());
    commands.entity(root).observe(
        move |closed: On<DialogClosed>, account: Res<Account>, mut calls: Query<&mut HubCalls>| {
            if closed.action != Some(1) {
                return;
            }
            let name = name.clone();
            call_hub(&account, &mut calls, move |client, session| {
                Answer::Changed {
                    result: client.delete_realm(session, id),
                    done: format!("Deleted \"{name}\"."),
                    focus: next,
                    field: None,
                    dialog: None,
                }
            });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::menus::worlds::INVALID_SEED;

    /// The hub's copy of the world settings a hosted world may have.
    mod agent_allowlist {
        include!("../../../../../hub/agent-protocol/src/allowlist.rs");
    }

    #[test]
    fn the_settings_the_form_offers_are_those_the_hub_allows() {
        let names = |choices: Vec<HostedChoices>| -> Vec<RealmSettings> {
            choices
                .iter()
                .map(|choices| settings_of(choices, None))
                .collect()
        };
        let world_types: Vec<String> = names(
            WORLD_TYPES
                .iter()
                .map(|(_, preset)| HostedChoices {
                    preset: preset.clone(),
                    ..default()
                })
                .collect(),
        )
        .into_iter()
        .filter_map(|settings| settings.world_type)
        .collect();
        assert_eq!(world_types, agent_allowlist::WORLD_TYPES);
        let game_modes: Vec<String> = names(
            GAME_MODES
                .iter()
                .map(|&(_, game_mode)| HostedChoices {
                    game_mode,
                    ..default()
                })
                .collect(),
        )
        .into_iter()
        .filter_map(|settings| settings.game_mode)
        .collect();
        assert_eq!(game_modes, agent_allowlist::GAME_MODES);
        let difficulties: Vec<String> = names(
            DIFFICULTIES
                .iter()
                .map(|&(_, difficulty)| HostedChoices {
                    difficulty,
                    ..default()
                })
                .collect(),
        )
        .into_iter()
        .filter_map(|settings| settings.difficulty)
        .collect();
        assert_eq!(difficulties, agent_allowlist::DIFFICULTIES);
    }

    #[test]
    fn a_new_world_is_sent_with_its_seed_and_its_choices() {
        let choices = HostedChoices {
            game_mode: GameMode::Creative,
            difficulty: Difficulty::Hard,
            preset: WorldPreset::LargeBiomes,
        };
        assert_eq!(
            settings_of(&choices, Some(42)),
            RealmSettings {
                seed: Some(42),
                world_type: Some("large-biomes".into()),
                game_mode: Some("creative".into()),
                difficulty: Some("hard".into()),
            }
        );
    }

    #[test]
    fn the_name_and_the_seed_are_checked_as_the_hub_would() {
        assert_eq!(check_create("  Home ", ""), ([None, None], true));
        assert_eq!(check_create("   ", "").0[0], Some(EMPTY_WORLD_NAME));
        assert_eq!(check_create(&"é".repeat(33), "").0[0], Some(INVALID_NAME));
        assert_eq!(check_create("Bell\u{7}", "").0[0], Some(INVALID_NAME));
        assert_eq!(
            check_create("Home", "99999999999"),
            ([None, Some(INVALID_SEED)], false)
        );
    }
}
