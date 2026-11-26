//! The Account dialog of the main menu: log in or create an account on the hub, recover an
//! account with its recovery code, or log out. The calls to the hub run on a thread; the dialog
//! stays usable and shows the answer when it comes.
//!
//! Bevy's `EditableText` has no password masking (its docs list it as not implemented), so a
//! password field keeps the real text in the field, drawn transparent, and a sibling text shows
//! one bullet per character over it.

use std::time::{Duration, Instant};

use bevy::{
    clipboard::Clipboard,
    input_focus::{tab_navigation::TabIndex, InputFocus},
    prelude::*,
    text::{EditableText, EditableTextSystems, TextCursorStyle, TextEdit},
    ui::{InteractionDisabled, UiSystems},
    ui_widgets::Activate,
};
use rustcraft_protocol::api::Me;
use shared::GameFolderPaths;
use uuid::Uuid;

use crate::account::file::{unix_now, AccountFile};
use crate::account::hub::{HubClient, HubError};
use crate::account::{Account, ExplicitPlayerName, HubUrl, Renewal};
use crate::constants::ACCOUNT_PATH;
use crate::network::status::InFlight;
use crate::network::CurrentPlayerProfile;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::menus::player_name::{open_player_name_dialog, PlayerNameButton};
use crate::ui::theme::{
    ACCENT, BEVEL_WIDTH, FONT_L, FONT_M, FONT_S, FONT_XL, MUTED, SPACE_L, SPACE_S, TEXT,
};
use crate::ui::widgets::button::{button, set_label, ButtonVariant};
use crate::ui::widgets::dialog::{
    apply_form_check, close_dialog, set_dialog_width, spawn_dialog, spawn_form_dialog,
    CloseRequest, Dialog, DialogClosed, DialogPanel, DialogSubmit, FormChanged,
};
use crate::ui::widgets::hints::Hint;
use crate::ui::widgets::tabs::{spawn_tabs, TabChange, Tabs};
use crate::ui::widgets::text_field::{
    set_text, spawn_text_field, value_of, FieldError, TextField, TextFieldParts,
};
use crate::ui::widgets::toast::{show_toast, ToastKind};

/// The title of the Account dialog.
pub const ACCOUNT_TITLE: &str = "Account";
/// The title of the dialog that shows the recovery code.
pub const RECOVERY_CODE_TITLE: &str = "Your recovery code";
/// The title of the dialog that recovers an account.
pub const RECOVER_TITLE: &str = "Recover account";

const LOG_IN: &str = "Log in";
const CREATE_ACCOUNT: &str = "Create account";
const WROTE_IT_DOWN: &str = "I wrote it down";
const PLAY_OFFLINE: &str = "Play offline as...";

/// The password lengths the hub accepts, in characters.
const MIN_PASSWORD: usize = 10;
const MAX_PASSWORD: usize = 128;
/// The username lengths the hub accepts.
const USERNAME_LENGTHS: std::ops::RangeInclusive<usize> = 3..=20;
/// The longest a recovery code is typed: `RCV-` and 3 groups of 5, with room for spaces.
const MAX_RECOVERY_CODE: usize = 32;

/// How long the answer of the hub is waited for. The hub client gives up after 10 s, so a call
/// still there after this one has lost its thread.
const CALL_DEADLINE: Duration = Duration::from_secs(30);

const FORM_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Submit",
    },
    Hint {
        keys: "Esc",
        action: "Cancel",
    },
];

/// A dialog to open on the next frame, once the one before it has closed and given the focus
/// back.
#[derive(Clone)]
pub enum Screen {
    /// The Account dialog: the log-in form, or the state of the account when logged in.
    Account,
    Recover,
    PlayerName,
    /// The recovery code of an account, to write down.
    RecoveryCode(String),
}

#[derive(Resource, Default)]
pub struct NextDialog(pub Option<Screen>);

/// What a dialog opens when it closes.
#[derive(Component)]
struct Follow(Screen);

/// What the form does with its fields.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FormKind {
    LogIn,
    Create,
    Recover,
}

/// The fields of a form of the Account screen, on the root of its dialog. The inputs are in the
/// order of the fields: username, then password and its confirmation, or recovery code and new
/// password.
#[derive(Component)]
struct AccountForm {
    kind: FormKind,
    inputs: Vec<Entity>,
    primary: Entity,
    /// Whether a call to the hub is waiting for an answer.
    busy: bool,
    /// The field that only creating asks for, and the link that only logging in has.
    confirm: Option<Entity>,
    forgot: Option<Entity>,
}

/// The dialog of a logged-in account.
#[derive(Component)]
struct SignedIn;

/// The dialog that shows a recovery code.
#[derive(Component)]
struct ShownCode(String);

/// The line of the certificate of the signed-in dialog.
#[derive(Component)]
struct CertificateLine;

/// The input of a password: its overlay shows one bullet per character.
#[derive(Component)]
struct Masked {
    overlay: Entity,
}

/// What the hub answered to a log-in, a sign-up or a recovery.
struct Answer {
    account: rustcraft_protocol::api::AccountSummary,
    session: String,
    recovery_code: Option<String>,
}

/// The call to the hub in progress, if any: one at a time.
#[derive(Resource, Default)]
struct Call(Option<Pending>);

/// The question to the hub whether the session of the account is still good, asked when the
/// dialog of a logged-in account opens.
#[derive(Resource, Default)]
struct SessionCheck(Option<(Uuid, InFlight<Result<Me, HubError>>)>);

struct Pending {
    /// The dialog the call was made from, if it is still open when the answer comes.
    form: Entity,
    kind: FormKind,
    hub: String,
    sent: Instant,
    request: InFlight<Result<Answer, HubError>>,
}

pub fn account_screen_plugin(app: &mut App) {
    app.init_resource::<NextDialog>()
        .init_resource::<Call>()
        .init_resource::<SessionCheck>()
        .add_observer(check_form)
        .add_observer(submit_form)
        .add_systems(
            Update,
            (
                open_next_dialog
                    .run_if(resource_exists::<Account>.and_then(resource_exists::<HubUrl>)),
                poll_call.run_if(resource_exists::<Account>.and_then(resource_exists::<Renewal>)),
                sync_forms,
                end_unauthorized_session.run_if(resource_exists::<Account>),
                show_certificate.run_if(resource_changed::<Account>),
                show_name_on_button.run_if(resource_changed::<CurrentPlayerProfile>),
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            keep_passwords_out_of_the_clipboard
                .before(EditableTextSystems)
                .before(UiSystems::Layout),
        )
        .add_systems(
            PostUpdate,
            show_bullets
                .after(EditableTextSystems)
                .before(UiSystems::Layout),
        );
}

/// The name button of the main menu opens the Account dialog, or the name dialog when the name
/// was given on the command line.
pub fn open_account_on_activate(
    _: On<Activate>,
    explicit: Option<Res<ExplicitPlayerName>>,
    mut next: ResMut<NextDialog>,
) {
    next.0 = Some(if explicit.is_some() {
        Screen::PlayerName
    } else {
        Screen::Account
    });
}

/// The host of a hub URL, to say it to people: `hub.rustcraft.dev`.
fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split('/').next().unwrap_or(rest)
}

/// Opens what `NextDialog` holds, a frame after the dialog before it closed.
fn open_next_dialog(
    mut next: ResMut<NextDialog>,
    mut session_check: ResMut<SessionCheck>,
    account: Res<Account>,
    profile: Res<CurrentPlayerProfile>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    let Some(screen) = next.0.take() else {
        return;
    };
    let font: Handle<Font> = assets.load(FONT_PATH);
    match screen {
        Screen::Account => match &*account {
            Account::LoggedOut => open_form(&mut commands, FormKind::LogIn, font, &focus),
            Account::LoggedIn(file) => {
                let (client, session, id) = (
                    HubClient::new(&file.hub),
                    file.session.clone(),
                    file.account,
                );
                session_check.0 = Some((
                    id,
                    InFlight::spawn(
                        Err(HubError::Unreachable("no thread to ask on".into())),
                        move || client.me(&session),
                    ),
                ));
                open_signed_in(&mut commands, file, font, &focus);
            }
        },
        Screen::Recover => open_form(&mut commands, FormKind::Recover, font, &focus),
        Screen::PlayerName => {
            open_player_name_dialog(&mut commands, &assets, &focus, Some(&profile.name));
        }
        Screen::RecoveryCode(code) => open_recovery_code(&mut commands, code, &assets, &focus),
    }
}

/// A text field of the forms.
fn field(
    commands: &mut Commands,
    label: &str,
    max_characters: usize,
    font: &Handle<Font>,
    masked: bool,
) -> TextFieldParts {
    let parts = spawn_text_field(
        commands,
        &TextField {
            label,
            placeholder: "",
            max_characters: Some(max_characters),
            font: font.clone(),
            font_size: FONT_M,
            reserve_error: true,
        },
    );
    if masked {
        // the real text is there, but invisible, with its cursor and selection
        let overlay = commands
            .spawn((
                Text::default(),
                TextFont {
                    font: font.clone().into(),
                    font_size: px(FONT_M).into(),
                    ..default()
                },
                TextColor(TEXT),
                TextLayout {
                    linebreak: LineBreak::NoWrap,
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: px(SPACE_S),
                    top: px(SPACE_S),
                    ..default()
                },
                // a click on the bullets reaches the field under them
                Pickable::IGNORE,
                ChildOf(parts.frame),
            ))
            .id();
        commands.entity(parts.input).insert((
            Masked { overlay },
            TextColor(Color::NONE),
            TextCursorStyle {
                color: Color::NONE,
                selection_color: Color::NONE,
                unfocused_selection_color: Color::NONE,
                selected_text_color: None,
            },
        ));
    }
    parts
}

/// Opens the log-in form (with its tabs), or the form of the recovery.
fn open_form(commands: &mut Commands, kind: FormKind, font: Handle<Font>, focus: &InputFocus) {
    let recover = kind == FormKind::Recover;
    let username = field(commands, "Username", *USERNAME_LENGTHS.end(), &font, false);
    let (second, third) = if recover {
        (
            field(commands, "Recovery code", MAX_RECOVERY_CODE, &font, false),
            field(commands, "New password", MAX_PASSWORD, &font, true),
        )
    } else {
        (
            field(commands, "Password", MAX_PASSWORD, &font, true),
            field(commands, "Confirm password", MAX_PASSWORD, &font, true),
        )
    };
    let (title, body, actions): (_, _, &[(&str, ButtonVariant)]) = if recover {
        (
            RECOVER_TITLE,
            "Your recovery code sets a new password.",
            &[
                ("Cancel", ButtonVariant::Default),
                ("Recover", ButtonVariant::Primary),
            ],
        )
    } else {
        (
            ACCOUNT_TITLE,
            "Log in to play with your account.",
            &[
                (PLAY_OFFLINE, ButtonVariant::Default),
                (LOG_IN, ButtonVariant::Primary),
            ],
        )
    };
    let dialog = Dialog {
        title,
        body,
        actions,
        focused: 0,
    };
    let fields: Vec<(&str, TextFieldParts)> = if recover {
        vec![
            ("Username", username),
            ("Recovery code", second),
            ("New password", third),
        ]
    } else {
        vec![
            ("Username", username),
            ("Password", second),
            ("Confirm password", third),
        ]
    };
    let parts = spawn_form_dialog(commands, &dialog, &fields, &FORM_HINTS, font.clone(), focus);
    let root = parts.root;
    let primary = parts.actions[1];
    let (mut confirm, mut forgot) = (None, None);
    if !recover {
        confirm = Some(third.root);
        let form = parts.form.expect("a form dialog has a form column");
        let tabs = spawn_tabs(
            commands,
            &Tabs {
                label: ACCOUNT_TITLE,
                tabs: &[LOG_IN, CREATE_ACCOUNT],
                selected: 0,
            },
            font.clone(),
        );
        commands.entity(form).insert_children(0, &[tabs]);
        commands.entity(tabs).observe(
            move |change: On<TabChange>,
                  mut forms: Query<&mut AccountForm>,
                  mut texts: Query<&mut EditableText>,
                  mut commands: Commands| {
                let Ok(mut form) = forms.get_mut(root) else {
                    return;
                };
                form.kind = if change.index == 0 {
                    FormKind::LogIn
                } else {
                    FormKind::Create
                };
                // what was typed for one tab is not an error of the other
                for &input in &form.inputs {
                    commands.entity(input).insert(FieldError(None));
                }
                if let Some(&confirm) = form.inputs.get(2) {
                    if let Ok(mut text) = texts.get_mut(confirm) {
                        set_text(&mut text, "");
                    }
                }
            },
        );
        let link = commands
            .spawn((
                button("Forgot password?", ButtonVariant::Ghost, font.clone()),
                ChildOf(form),
            ))
            .observe(move |_: On<Activate>, mut commands: Commands| {
                commands.entity(root).insert(Follow(Screen::Recover));
                close_dialog(&mut commands, root, true);
            })
            .id();
        commands
            .entity(link)
            .entry::<Node>()
            .and_modify(|mut node| {
                node.align_self = AlignSelf::FlexStart;
                // the padding and bevel of the button do not push its text off the fields
                node.margin = UiRect::left(px(-SPACE_L - BEVEL_WIDTH));
            });
        forgot = Some(link);
    }
    commands
        .entity(root)
        .insert(AccountForm {
            kind,
            inputs: fields.iter().map(|(_, field)| field.input).collect(),
            primary,
            busy: false,
            confirm,
            forgot,
        })
        .observe(follow_on_close);
}

/// Opens what the closed dialog asked for: the link of a form, or "Play offline as...".
fn follow_on_close(
    closed: On<DialogClosed>,
    follows: Query<&Follow>,
    forms: Query<&AccountForm>,
    mut next: ResMut<NextDialog>,
) {
    if let Ok(Follow(screen)) = follows.get(closed.entity) {
        next.0 = Some(screen.clone());
    } else if let Ok(form) = forms.get(closed.entity) {
        if form.kind != FormKind::Recover && closed.action == Some(0) {
            next.0 = Some(Screen::PlayerName);
        }
    }
}

/// What is wrong with the fields typed, for each of them, and whether the form can be
/// submitted. The hub is the authority: this is for an instant answer.
fn check(kind: FormKind, values: &[String]) -> (Vec<Option<&'static str>>, bool) {
    let [username, second, third] = values else {
        return (vec![None; values.len()], false);
    };
    let short_password = |password: &str| {
        (password.chars().count() < MIN_PASSWORD).then_some("At least 10 characters")
    };
    let errors = match kind {
        FormKind::LogIn => vec![
            username.trim().is_empty().then_some("Enter your username"),
            second.is_empty().then_some("Enter your password"),
            None,
        ],
        FormKind::Create => vec![
            (!USERNAME_LENGTHS.contains(&username.chars().count())
                || !username
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_'))
            .then_some("3 to 20 letters, digits or _"),
            short_password(second),
            (second != third).then_some("Passwords differ"),
        ],
        FormKind::Recover => vec![
            username.trim().is_empty().then_some("Enter your username"),
            second
                .trim()
                .is_empty()
                .then_some("Enter your recovery code"),
            short_password(third),
        ],
    };
    let submittable = errors.iter().all(Option::is_none)
        // an empty field has no error to show before it was typed in
        && !username.trim().is_empty()
        && !second.trim().is_empty()
        && (kind == FormKind::LogIn || !third.is_empty());
    (errors, submittable)
}

/// Shows why a typed field is wrong, once it was typed in, and disables the primary action while
/// the form cannot be submitted or waits for the hub.
fn check_form(changed: On<FormChanged>, forms: Query<&AccountForm>, mut commands: Commands) {
    let Ok(form) = forms.get(changed.entity) else {
        return;
    };
    let (errors, submittable) = check(form.kind, &changed.values);
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

/// Puts the form in the state of its tab: the confirmation and the link that go with it, the
/// label and availability of the primary action.
fn sync_forms(
    forms: Query<&AccountForm, Changed<AccountForm>>,
    texts: Query<&EditableText>,
    parents: Query<&ChildOf>,
    mut nodes: Query<&mut Node>,
    mut commands: Commands,
) {
    for form in &forms {
        let mut show = |shown: bool, display: Entity, control: Entity| {
            if let Ok(mut node) = nodes.get_mut(display) {
                let display = if shown { Display::Flex } else { Display::None };
                if node.display != display {
                    node.display = display;
                }
            }
            // a hidden control is no stop of the Tab key
            commands
                .entity(control)
                .insert(TabIndex(if shown { 0 } else { -1 }));
        };
        if let Some(confirm) = form.confirm {
            // the field sits in a column with its caption
            let column = parents.get(confirm).map_or(confirm, ChildOf::parent);
            show(form.kind == FormKind::Create, column, form.inputs[2]);
        }
        if let Some(forgot) = form.forgot {
            show(form.kind == FormKind::LogIn, forgot, forgot);
        }
        let label = match form.kind {
            FormKind::LogIn => LOG_IN,
            FormKind::Create => CREATE_ACCOUNT,
            FormKind::Recover => "Recover",
        };
        commands.entity(form.primary).queue(set_label(label));
        let values: Vec<String> = form.inputs.iter().map(|&i| value_of(&texts, i)).collect();
        if check(form.kind, &values).1 && !form.busy {
            commands
                .entity(form.primary)
                .remove::<InteractionDisabled>();
        } else {
            commands.entity(form.primary).insert(InteractionDisabled);
        }
    }
}

/// Sends the form to the hub: logs in, creates the account or recovers it.
fn submit_form(
    submit: On<DialogSubmit>,
    mut forms: Query<&mut AccountForm, Without<CloseRequest>>,
    texts: Query<&EditableText>,
    hub: Option<Res<HubUrl>>,
    mut call: ResMut<Call>,
) {
    let (Some(hub), Ok(mut form)) = (hub, forms.get_mut(submit.entity)) else {
        return;
    };
    if form.busy || call.0.is_some() {
        return;
    }
    let values: Vec<String> = form.inputs.iter().map(|&i| value_of(&texts, i)).collect();
    if !check(form.kind, &values).1 {
        return;
    }
    let client = HubClient::new(&hub.0);
    let kind = form.kind;
    let [username, second, third] = [0, 1, 2].map(|i| values[i].clone());
    let request = InFlight::spawn(
        Err(HubError::Unreachable("no thread to ask on".into())),
        move || {
            let username = username.trim();
            match kind {
                FormKind::LogIn => client.log_in(username, &second, None).map(|r| Answer {
                    account: r.account,
                    session: r.session,
                    recovery_code: None,
                }),
                FormKind::Create => {
                    client
                        .create_account(username, &second, None)
                        .map(|r| Answer {
                            account: r.account,
                            session: r.session,
                            recovery_code: Some(r.recovery_code),
                        })
                }
                FormKind::Recover => {
                    client
                        .recover(username, second.trim(), &third)
                        .map(|r| Answer {
                            account: r.account,
                            session: r.session,
                            recovery_code: Some(r.recovery_code),
                        })
                }
            }
        },
    );
    form.busy = true;
    call.0 = Some(Pending {
        form: submit.entity,
        kind,
        hub: hub.0.clone(),
        sent: Instant::now(),
        request,
    });
}

/// What to show of a refusal of the hub.
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    /// Under the field `index` of the form.
    Field(usize, String),
    Toast(String),
}

/// The refusal `error` of the hub for a form of `kind` made at `host`.
fn failure(error: &HubError, kind: FormKind, host: &str) -> Failure {
    use rustcraft_protocol::api::ErrorCode::*;
    match error {
        HubError::Api { code, message } => {
            let field = match (code, kind) {
                (InvalidUsername | UsernameTaken, _) => Some(0),
                (BadCredentials, _) => Some(1),
                (InvalidPassword | WeakPassword, FormKind::Recover) => Some(2),
                (InvalidPassword | WeakPassword, _) => Some(1),
                _ => None,
            };
            match field {
                Some(index) => Failure::Field(index, message.clone()),
                None => Failure::Toast(message.clone()),
            }
        }
        HubError::Unreachable(_) => Failure::Toast(format!("Cannot reach the hub at {host}.")),
        HubError::Malformed(_) => Failure::Toast(error.to_string()),
    }
}

/// Takes the answer of the hub when it came: the account is logged in, or the failure is shown.
#[allow(clippy::too_many_arguments)]
fn poll_call(
    mut call: ResMut<Call>,
    mut account: ResMut<Account>,
    mut renewal: ResMut<Renewal>,
    mut forms: Query<&mut AccountForm>,
    paths: Res<GameFolderPaths>,
    assets: Res<AssetServer>,
    mut next: ResMut<NextDialog>,
    mut commands: Commands,
) {
    let Some(pending) = &call.0 else {
        return;
    };
    let Some(result) = pending.request.take() else {
        if pending.sent.elapsed() > CALL_DEADLINE {
            warn!("A call to the hub never came back, forgetting it");
            let form = pending.form;
            call.0 = None;
            if let Ok(mut form) = forms.get_mut(form) {
                form.busy = false;
            }
        }
        return;
    };
    let Pending {
        form, kind, hub, ..
    } = call.0.take().expect("a call was pending");
    let font: Handle<Font> = assets.load(FONT_PATH);
    let open = forms.contains(form);
    if let Ok(mut form) = forms.get_mut(form) {
        form.busy = false;
    }
    match result {
        Err(error) => {
            warn!("The hub refused or did not answer: {error}");
            match failure(&error, kind, host_of(&hub)) {
                Failure::Field(index, message) if open => {
                    let input = forms.get(form).map(|f| f.inputs[index]);
                    if let Ok(input) = input {
                        commands.entity(input).insert(FieldError(Some(message)));
                    }
                }
                Failure::Field(_, message) | Failure::Toast(message) => {
                    show_toast(&mut commands, &message, ToastKind::Error, font);
                }
            }
        }
        Ok(answer) => {
            let file = AccountFile::new(&hub, &answer.account, answer.session);
            if let Err(e) = file.save(&paths.game_folder_path.join(ACCOUNT_PATH)) {
                error!("Cannot save {ACCOUNT_PATH}: {e}");
                show_toast(
                    &mut commands,
                    "Could not save your account on this computer.",
                    ToastKind::Error,
                    font.clone(),
                );
            }
            let name = file.username.clone();
            *account = Account::LoggedIn(file);
            // the certificate is needed to join a server: ask at once
            renewal.ask_now();
            if open {
                close_dialog(&mut commands, form, true);
            }
            match answer.recovery_code {
                Some(code) => next.0 = Some(Screen::RecoveryCode(code)),
                None => {
                    show_toast(
                        &mut commands,
                        &format!("Logged in as {name}."),
                        ToastKind::Info,
                        font,
                    );
                }
            }
        }
    }
}

/// Adds `content` to the panel of the dialog `root`, above its buttons.
pub(crate) fn add_to_panel(commands: &mut Commands, root: Entity, content: Vec<Entity>) {
    commands.queue(move |world: &mut World| {
        let panel = world.get::<Children>(root).and_then(|children| {
            children
                .iter()
                .find(|&child| world.get::<DialogPanel>(child).is_some())
        });
        let Some(panel) = panel else {
            return;
        };
        let at = world
            .get::<Children>(panel)
            .map_or(0, |children| children.len().saturating_sub(1));
        world.entity_mut(panel).insert_children(at, &content);
    });
}

fn text_bundle(
    content: &str,
    size: f32,
    color: Color,
    font: Handle<Font>,
) -> (Text, TextFont, TextColor, TextLayout) {
    (
        Text::new(content),
        TextFont {
            font: font.into(),
            font_size: px(size).into(),
            ..default()
        },
        TextColor(color),
        TextLayout::linebreak(LineBreak::WordOrCharacter),
    )
}

/// The dialog that shows the recovery code once. It has no way out but its button: closing it
/// otherwise shows it again, as the code is not shown twice.
fn open_recovery_code(
    commands: &mut Commands,
    code: String,
    assets: &AssetServer,
    focus: &InputFocus,
) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let dialog = Dialog {
        title: RECOVERY_CODE_TITLE,
        body: "It is the only way back into your account if you forget your password. \
               It is shown once: write it down.",
        actions: &[(WROTE_IT_DOWN, ButtonVariant::Primary)],
        focused: 0,
    };
    let root = spawn_dialog(commands, &dialog, font.clone(), focus);
    set_dialog_width(commands, root, 520.0);
    let shown = commands
        .spawn(text_bundle(
            &code,
            FONT_XL,
            ACCENT,
            assets.load(MONO_FONT_PATH),
        ))
        .id();
    let copied = code.clone();
    let copy = commands
        .spawn(button("Copy", ButtonVariant::Default, font))
        .observe(
            move |_: On<Activate>,
                  clipboard: Option<ResMut<Clipboard>>,
                  assets: Res<AssetServer>,
                  mut commands: Commands| {
                let done = clipboard
                    .is_some_and(|mut clipboard| clipboard.set_text(copied.clone()).is_ok());
                let (message, kind) = if done {
                    ("Copied.", ToastKind::Info)
                } else {
                    ("Cannot copy. Write it down.", ToastKind::Error)
                };
                show_toast(&mut commands, message, kind, assets.load(FONT_PATH));
            },
        )
        .id();
    commands
        .entity(copy)
        .entry::<Node>()
        .and_modify(|mut node| node.align_self = AlignSelf::FlexStart);
    commands
        .entity(root)
        .insert(ShownCode(code))
        .observe(show_code_again);
    add_to_panel(commands, root, vec![shown, copy]);
}

/// Shows the code again when the dialog was closed by Escape.
fn show_code_again(
    closed: On<DialogClosed>,
    codes: Query<&ShownCode>,
    mut next: ResMut<NextDialog>,
) {
    if closed.action.is_none() {
        if let Ok(ShownCode(code)) = codes.get(closed.entity) {
            next.0 = Some(Screen::RecoveryCode(code.clone()));
        }
    }
}

/// The certificate of the account, in a line.
fn certificate_line(file: &AccountFile) -> String {
    match file.expires_at {
        None => "Certificate: asking the hub".to_string(),
        Some(expires_at) if expires_at <= unix_now() => "Certificate expired".to_string(),
        Some(expires_at) => {
            let when = chrono::DateTime::from_timestamp(i64::try_from(expires_at).unwrap_or(0), 0)
                .map(|when| when.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_default();
            format!("Certificate valid until {when}")
        }
    }
}

/// The dialog of a logged-in account: who, where, and its certificate.
fn open_signed_in(
    commands: &mut Commands,
    file: &AccountFile,
    font: Handle<Font>,
    focus: &InputFocus,
) {
    let dialog = Dialog {
        title: ACCOUNT_TITLE,
        body: "",
        actions: &[
            ("Log out", ButtonVariant::Danger),
            ("Close", ButtonVariant::Primary),
        ],
        focused: 1,
    };
    let root = spawn_dialog(commands, &dialog, font.clone(), focus);
    let lines = [
        commands
            .spawn(text_bundle(&file.username, FONT_L, TEXT, font.clone()))
            .id(),
        commands
            .spawn(text_bundle(
                &format!("Signed in to {}", host_of(&file.hub)),
                FONT_M,
                MUTED,
                font.clone(),
            ))
            .id(),
        commands
            .spawn((
                text_bundle(&certificate_line(file), FONT_S, MUTED, font),
                CertificateLine,
            ))
            .id(),
    ];
    commands
        .entity(root)
        .insert(SignedIn)
        .observe(log_out_on_close);
    add_to_panel(commands, root, lines.to_vec());
}

/// Keeps the certificate line of the open dialog up to date: it comes some time after the log-in.
fn show_certificate(account: Res<Account>, mut lines: Query<&mut Text, With<CertificateLine>>) {
    let Account::LoggedIn(file) = &*account else {
        return;
    };
    let line = certificate_line(file);
    for mut text in &mut lines {
        if text.0 != line {
            text.0.clone_from(&line);
        }
    }
}

/// Logs out when the dialog closes with "Log out": the hub is told on a thread, and what is kept
/// on this computer goes whatever it answers.
fn log_out_on_close(
    closed: On<DialogClosed>,
    dialogs: Query<(), With<SignedIn>>,
    mut account: ResMut<Account>,
    paths: Res<GameFolderPaths>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    if closed.action != Some(0) || !dialogs.contains(closed.entity) {
        return;
    }
    let Account::LoggedIn(file) = &*account else {
        return;
    };
    let (client, session) = (HubClient::new(&file.hub), file.session.clone());
    // nothing waits for the answer: the thread ends with the call
    drop(InFlight::spawn((), move || {
        if let Err(e) = client.log_out(&session) {
            warn!("The hub was not told of the log-out: {e}");
        }
    }));
    if let Err(e) = AccountFile::forget(&paths.game_folder_path.join(ACCOUNT_PATH)) {
        error!("Cannot delete {ACCOUNT_PATH}: {e}");
    }
    *account = Account::LoggedOut;
    show_toast(
        &mut commands,
        "Logged out.",
        ToastKind::Info,
        assets.load(FONT_PATH),
    );
}

/// Shows the name of the player on the button of the main menu.
fn show_name_on_button(
    profile: Res<CurrentPlayerProfile>,
    buttons: Query<Entity, With<PlayerNameButton>>,
    mut commands: Commands,
) {
    for button in &buttons {
        commands.entity(button).queue(set_label(&profile.name));
    }
}

/// Drops the Copy and Cut edits queued by Ctrl+C, Ctrl+X and their other keys: they would put
/// the real password, which the field holds under its bullets, on the clipboard.
fn without_copy_and_cut(edits: &mut Vec<TextEdit>) {
    edits.retain(|edit| !matches!(edit, TextEdit::Copy | TextEdit::Cut));
}

/// Keeps the passwords of the masked fields out of the clipboard. Typing and pasting go on.
fn keep_passwords_out_of_the_clipboard(mut fields: Query<&mut EditableText, With<Masked>>) {
    for mut field in &mut fields {
        if field
            .pending_edits
            .iter()
            .any(|edit| matches!(edit, TextEdit::Copy | TextEdit::Cut))
        {
            without_copy_and_cut(&mut field.pending_edits);
        }
    }
}

/// One bullet per character of the password, over its transparent field.
fn show_bullets(
    fields: Query<(&EditableText, &Masked), Changed<EditableText>>,
    mut texts: Query<&mut Text>,
) {
    for (field, masked) in &fields {
        let Ok(mut text) = texts.get_mut(masked.overlay) else {
            continue;
        };
        let count = field.value().to_string().chars().count();
        if text.0.chars().count() != count {
            text.0 = "•".repeat(count);
        }
    }
}

/// Logs out an account whose session the hub no longer knows (it was revoked, or expired): there
/// is nothing to do with it but log in again.
fn end_unauthorized_session(
    mut check: ResMut<SessionCheck>,
    mut account: ResMut<Account>,
    paths: Res<GameFolderPaths>,
    assets: Res<AssetServer>,
    dialogs: Query<Entity, With<SignedIn>>,
    mut commands: Commands,
) {
    let Some((id, request)) = &check.0 else {
        return;
    };
    let Some(answer) = request.take() else {
        return;
    };
    let id = *id;
    check.0 = None;
    let Err(error) = answer else {
        return;
    };
    let current = matches!(&*account, Account::LoggedIn(file) if file.account == id);
    if !error.is_unauthorized() || !current {
        if !error.is_unauthorized() {
            warn!("Cannot check the session of the account: {error}");
        }
        return;
    }
    if let Err(e) = AccountFile::forget(&paths.game_folder_path.join(ACCOUNT_PATH)) {
        error!("Cannot delete {ACCOUNT_PATH}: {e}");
    }
    *account = Account::LoggedOut;
    for dialog in &dialogs {
        close_dialog(&mut commands, dialog, true);
    }
    show_toast(
        &mut commands,
        "Your session ended. Log in again.",
        ToastKind::Error,
        assets.load(FONT_PATH),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_protocol::api::ErrorCode;

    fn values(a: &str, b: &str, c: &str) -> Vec<String> {
        [a, b, c].map(str::to_string).to_vec()
    }

    #[test]
    fn copy_and_cut_are_dropped_from_the_edits_of_a_password() {
        let mut edits = vec![
            TextEdit::Copy,
            TextEdit::Insert("a".into()),
            TextEdit::Cut,
            TextEdit::SelectAll,
        ];
        without_copy_and_cut(&mut edits);
        assert_eq!(edits, [TextEdit::Insert("a".into()), TextEdit::SelectAll]);
    }

    #[test]
    fn a_hub_is_named_by_its_host() {
        assert_eq!(host_of("https://hub.rustcraft.dev"), "hub.rustcraft.dev");
        assert_eq!(host_of("http://127.0.0.1:8080/"), "127.0.0.1:8080");
        assert_eq!(host_of("hub.example"), "hub.example");
    }

    #[test]
    fn logging_in_needs_a_username_and_a_password() {
        assert!(!check(FormKind::LogIn, &values("", "", "")).1);
        assert!(!check(FormKind::LogIn, &values("Steve", "", "")).1);
        assert!(check(FormKind::LogIn, &values("Steve", "x", "")).1);
    }

    #[test]
    fn creating_an_account_follows_the_rules_of_the_hub() {
        let long = "correct horse battery";
        assert!(check(FormKind::Create, &values("Steve", long, long)).1);
        let (errors, ok) = check(FormKind::Create, &values("St", "short", "other"));
        assert!(!ok);
        assert_eq!(
            errors,
            [
                Some("3 to 20 letters, digits or _"),
                Some("At least 10 characters"),
                Some("Passwords differ")
            ]
        );
        assert!(!check(FormKind::Create, &values("Steve Jobs", long, long)).1);
        assert!(!check(FormKind::Create, &values("Steve", long, "")).1);
    }

    #[test]
    fn recovering_needs_the_code_and_a_long_password() {
        assert!(
            check(
                FormKind::Recover,
                &values("Steve", "RCV-1", "a long enough one")
            )
            .1
        );
        assert!(!check(FormKind::Recover, &values("Steve", "", "a long enough one")).1);
        assert!(!check(FormKind::Recover, &values("Steve", "RCV-1", "short")).1);
    }

    fn api(code: ErrorCode) -> HubError {
        HubError::Api {
            code,
            message: "Nope.".into(),
        }
    }

    #[test]
    fn a_refusal_of_the_hub_goes_under_its_field_or_in_a_toast() {
        let field = |index| Failure::Field(index, "Nope.".into());
        assert_eq!(
            failure(&api(ErrorCode::UsernameTaken), FormKind::Create, "h"),
            field(0)
        );
        assert_eq!(
            failure(&api(ErrorCode::BadCredentials), FormKind::LogIn, "h"),
            field(1)
        );
        assert_eq!(
            failure(&api(ErrorCode::WeakPassword), FormKind::Recover, "h"),
            field(2)
        );
        assert_eq!(
            failure(&api(ErrorCode::RateLimited), FormKind::LogIn, "h"),
            Failure::Toast("Nope.".into())
        );
        assert_eq!(
            failure(
                &HubError::Unreachable("refused".into()),
                FormKind::LogIn,
                "hub.example"
            ),
            Failure::Toast("Cannot reach the hub at hub.example.".into())
        );
    }
}
