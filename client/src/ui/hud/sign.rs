//! The editor of a sign: placing a sign, or clicking one, opens it with the four lines the sign
//! says, and saving it writes them on the sign (`ClientToServerMessage::WriteSign`). The command
//! `/sign` still writes a line on the sign aimed at.

use bevy::{input_focus::InputFocus, prelude::*, text::EditableText};
use bevy_renet::RenetClient;
use shared::messages::{ClientToServerMessage, SIGN_LINES, SIGN_LINE_LENGTH};

use super::UiDialog;
use crate::i18n::{tr, trf};
use crate::network::SendGameMessageExtension;
use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::theme;
use crate::ui::widgets::button::ButtonVariant;
use crate::ui::widgets::dialog::{
    close_dialog, spawn_form_dialog, CloseRequest, Dialog, DialogSubmit,
};
use crate::ui::widgets::hints::Hint;
use crate::ui::widgets::text_field::{set_text, spawn_text_field, value_of, TextField};

/// The sign the server asked to edit, and what it says, until its editor opens.
#[derive(Resource, Default)]
pub struct SignToEdit(pub Option<(IVec3, [String; SIGN_LINES])>);

/// The root of the open editor: the sign it writes on, and its fields, one per line.
#[derive(Component)]
struct SignEditor {
    at: IVec3,
    inputs: [Entity; SIGN_LINES],
}

/// The key hints of the editor.
const SIGN_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Save",
    },
    Hint {
        keys: "Esc",
        action: "Cancel",
    },
];

pub fn sign_plugin(app: &mut App) {
    app.init_resource::<SignToEdit>()
        .add_systems(
            Update,
            open_sign_editor
                .run_if(resource_changed::<SignToEdit>)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(OnExit(GameState::Game), |mut sign: ResMut<SignToEdit>| {
            sign.0 = None;
        });
}

/// Opens the editor of the sign the server asked for, unless one is open already.
fn open_sign_editor(
    mut sign: ResMut<SignToEdit>,
    open: Query<(), With<SignEditor>>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    let Some((at, lines)) = sign.0.take() else {
        return;
    };
    if !open.is_empty() {
        return;
    }
    let font: Handle<Font> = assets.load(FONT_PATH);
    let fields = lines.map(|line| {
        let field = spawn_text_field(
            &mut commands,
            &TextField {
                label: tr("sign.line"),
                placeholder: "",
                max_characters: Some(SIGN_LINE_LENGTH),
                font: font.clone(),
                font_size: theme::font_m(),
                reserve_error: false,
            },
        );
        commands
            .entity(field.input)
            .entry::<EditableText>()
            .and_modify(move |mut text| set_text(&mut text, &line));
        field
    });
    let captions: [String; SIGN_LINES] =
        std::array::from_fn(|index| trf("sign.line_number", &[("number", &(index + 1))]));
    let labelled: Vec<(&str, _)> = captions
        .iter()
        .map(String::as_str)
        .zip(fields.iter().copied())
        .collect();
    let dialog = Dialog {
        title: tr("sign.title"),
        body: "",
        actions: &[
            (tr("common.cancel"), ButtonVariant::Default),
            (tr("common.save"), ButtonVariant::Primary),
        ],
        focused: 0,
    };
    let parts = spawn_form_dialog(&mut commands, &dialog, &labelled, &SIGN_HINTS, font, &focus);
    commands
        .entity(parts.root)
        .insert((
            SignEditor {
                at,
                inputs: fields.map(|field| field.input),
            },
            // the cursor is free while it is open, as for any dialog of the game
            UiDialog,
            Visibility::Visible,
            DespawnOnExit(GameState::Game),
        ))
        .observe(write_sign_on_submit);
}

/// Sends the lines of the editor to the server, then closes it. An editor already closing was
/// submitted before in this update.
fn write_sign_on_submit(
    submit: On<DialogSubmit>,
    editors: Query<&SignEditor, Without<CloseRequest>>,
    texts: Query<&EditableText>,
    mut client: ResMut<RenetClient>,
    mut commands: Commands,
) {
    let root = submit.entity;
    let Ok(editor) = editors.get(root) else {
        return;
    };
    let lines = editor.inputs.map(|input| value_of(&texts, input));
    client.send_game_message(ClientToServerMessage::WriteSign {
        at: editor.at,
        lines,
    });
    close_dialog(&mut commands, root, true);
}
