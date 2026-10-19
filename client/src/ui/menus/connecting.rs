//! The connecting card, between the menus and the game: the steps of the session being opened,
//! its progress, and Cancel (Escape), which goes back to the list the session came from.
//!
//! The card only uses the fonts the menus already hold, so it is laid out on its first frame.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input_focus::{tab_navigation::TabGroup, AutoFocus},
    prelude::*,
    ui_widgets::Activate,
};

use super::background::menu_background;
use super::text;
use crate::network::session::{
    end_session, progress, track_connection, ConnectStep, ConnectWatch, SessionOrigin,
};
use crate::network::LastWorldClosing;
use crate::state::GameState;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::menus::modal_open;
use crate::ui::theme::{
    BEVEL_HI, BEVEL_WIDTH, FONT_L, FONT_M, FONT_S, MUTED, OK, PANEL, SPACE_L, SPACE_S, SPACE_XL,
    SPACE_XS, TEXT,
};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::hints::{hint_bar, Hint};
use crate::ui::widgets::progress::{progress_bar, ProgressBar};
use crate::ui::widgets::raised_bevel;

/// The card: its title, its steps, its progress bar and Cancel.
#[derive(Component)]
pub struct ConnectingCard;

/// The row of the step `.0`, from 0.
#[derive(Component)]
pub struct StepRow(pub usize);

/// The Cancel button of the card.
#[derive(Component)]
pub struct CancelConnect;

/// The square before the label of a step, in its row.
#[derive(Component)]
struct StepMarker;

/// The label of a step, in its row.
#[derive(Component)]
struct StepLabel;

/// How many of the chunks around the spawn are in, on the row of the chunks.
#[derive(Component)]
struct ChunkCount;

/// The key hints of the card.
pub const CONNECT_HINTS: [Hint; 1] = [Hint {
    keys: "Esc",
    action: "Cancel",
}];

/// The width of the card, and of the error card that may take its place.
pub const CARD_WIDTH: f32 = 480.0;
/// The side of the square marker of a step.
const MARKER_SIDE: f32 = 10.0;
/// How long the marker of the current step shows, then hides, in seconds.
pub(crate) const BLINK: f32 = 0.5;
/// The row of the chunks.
const CHUNKS_ROW: usize = 2;
/// The label of the first step of a world opened while the last one closes.
pub const CLOSING_LAST_WORLD: &str = "Closing the last world...";

/// The title of the card: `Opening "Survie"` for a world, `Joining "Friends"` for a server.
pub fn connecting_title(origin: &SessionOrigin) -> String {
    match origin {
        SessionOrigin::World { name } => format!("Opening \"{name}\""),
        SessionOrigin::Server { name, .. } => format!("Joining \"{name}\""),
    }
}

/// The labels of the steps of a session, in order. The first row, the wait for the last world
/// to close, only shows on the card of a world that waits for it.
pub fn step_labels(origin: &SessionOrigin) -> [&'static str; 4] {
    let first = if origin.is_world() {
        "Starting the world"
    } else {
        "Connecting"
    };
    [CLOSING_LAST_WORLD, first, "Loading chunks", "Spawning"]
}

/// The row of `step`.
fn step_row(step: ConnectStep) -> usize {
    match step {
        ConnectStep::ClosingLastWorld => 0,
        ConnectStep::StartingWorld | ConnectStep::Connecting => 1,
        ConnectStep::LoadingChunks { .. } => CHUNKS_ROW,
        ConnectStep::Spawning => 3,
    }
}

pub fn connecting_plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            // the step of the frame: the card of a world waiting for the last one never shows
            // it starting first
            show_step
                .after(track_connection)
                .run_if(resource_exists_and_changed::<ConnectWatch>),
            blink_current_step,
            cancel_on_escape.run_if(not(modal_open)),
        )
            .run_if(in_state(GameState::PreGameLoading)),
    )
    .add_observer(cancel_on_activate);
}

/// Spawns the card of the session being opened, every step pending until `show_step`; with the
/// wait for the last world to close first when it does (`LastWorldClosing`).
pub fn setup_connecting_card(
    origin: Option<Res<SessionOrigin>>,
    closing: Option<Res<LastWorldClosing>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Some(origin) = origin else {
        error!("No session to show the connection of");
        return;
    };
    let font: Handle<Font> = assets.load(FONT_PATH);
    let title = connecting_title(&origin);
    let root = commands
        .spawn((
            menu_background(&assets),
            DespawnOnExit(GameState::PreGameLoading),
            TabGroup::new(0),
        ))
        .id();
    // centred in the window, as the error card that may take its place
    let centre = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let card = commands
        .spawn((
            ConnectingCard,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::Group)),
            AccessibleLabel::new(title.as_str()),
            Node {
                flex_direction: FlexDirection::Column,
                width: px(CARD_WIDTH),
                max_width: percent(90),
                padding: UiRect::all(px(SPACE_XL)),
                row_gap: px(SPACE_L),
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            BackgroundColor(PANEL),
            raised_bevel(),
            ChildOf(centre),
        ))
        .id();
    let heading = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_XS),
                ..default()
            },
            ChildOf(card),
            children![text(&title, FONT_L, TEXT, &font)],
        ))
        .id();
    if let SessionOrigin::Server { ip, .. } = &*origin {
        commands.spawn((text(ip, FONT_S, MUTED, &font), ChildOf(heading)));
    }
    let steps = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_S),
                ..default()
            },
            ChildOf(card),
        ))
        .id();
    let skipped = if closing.is_some() { 0 } else { 1 };
    for (index, label) in step_labels(&origin).into_iter().enumerate().skip(skipped) {
        let row = commands
            .spawn((
                StepRow(index),
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(SPACE_S),
                    ..default()
                },
                ChildOf(steps),
                children![
                    (
                        StepMarker,
                        Node {
                            width: px(MARKER_SIDE),
                            height: px(MARKER_SIDE),
                            flex_shrink: 0.0,
                            border: UiRect::all(px(BEVEL_WIDTH)),
                            ..default()
                        },
                        BorderColor::all(BEVEL_HI),
                    ),
                    (StepLabel, text(label, FONT_M, MUTED, &font)),
                ],
            ))
            .id();
        if index == CHUNKS_ROW {
            commands.spawn((
                ChunkCount,
                text("", FONT_M, MUTED, &font),
                Node {
                    margin: UiRect::left(auto()),
                    ..default()
                },
                ChildOf(row),
            ));
        }
    }
    commands.spawn((progress_bar("Progress"), ChildOf(card)));
    commands.spawn((
        Node {
            justify_content: JustifyContent::FlexEnd,
            ..default()
        },
        ChildOf(card),
        children![(
            button("Cancel", ButtonVariant::Default, font.clone()),
            CancelConnect,
            AutoFocus,
        )],
    ));
    commands.spawn((
        hint_bar(&CONNECT_HINTS, font, assets.load(MONO_FONT_PATH)),
        ChildOf(root),
    ));
}

/// Shows the step of the session: the steps before it done, it current, those after it
/// pending; the chunks in while they load, and the progress.
fn show_step(
    watch: Res<ConnectWatch>,
    rows: Query<&StepRow>,
    mut markers: Query<(&ChildOf, &mut BackgroundColor, &mut BorderColor), With<StepMarker>>,
    mut labels: Query<(&ChildOf, &mut TextColor), With<StepLabel>>,
    mut counts: Query<&mut Text, With<ChunkCount>>,
    mut bars: Query<&mut ProgressBar>,
) {
    let current = step_row(watch.step);
    let row_of = |child_of: &ChildOf| rows.get(child_of.parent()).map_or(0, |row| row.0);
    for (child_of, mut background, mut border) in &mut markers {
        let row = row_of(child_of);
        let (fill, edge) = if row < current {
            (OK, OK)
        } else if row == current {
            (TEXT, TEXT)
        } else {
            (Color::NONE, BEVEL_HI)
        };
        background.0 = fill;
        *border = BorderColor::all(edge);
    }
    for (child_of, mut color) in &mut labels {
        color.0 = if row_of(child_of) == current {
            TEXT
        } else {
            MUTED
        };
    }
    let count = match watch.step {
        ConnectStep::LoadingChunks { received, total } => format!("{received} / {total}"),
        _ => String::new(),
    };
    for mut text in &mut counts {
        text.0.clone_from(&count);
    }
    for mut bar in &mut bars {
        bar.0 = progress(watch.step);
    }
}

/// Blinks the marker of the current step: shown half a second, hidden half a second.
fn blink_current_step(
    time: Res<Time<Real>>,
    watch: Option<Res<ConnectWatch>>,
    rows: Query<&StepRow>,
    mut markers: Query<(&ChildOf, &mut Visibility), With<StepMarker>>,
) {
    let Some(watch) = watch else {
        return;
    };
    let current = step_row(watch.step);
    let shown = time.elapsed_secs() % (2.0 * BLINK) < BLINK;
    for (child_of, mut visibility) in &mut markers {
        let row = rows.get(child_of.parent()).map_or(0, |row| row.0);
        visibility.set_if_neq(if row == current && !shown {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
}

/// Cancel ends the session being opened, back to the list it came from.
fn cancel_on_activate(
    activate: On<Activate>,
    cancels: Query<(), With<CancelConnect>>,
    origin: Option<Res<SessionOrigin>>,
    mut game: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    if !cancels.contains(activate.entity) {
        return;
    }
    if let Some(origin) = origin {
        cancel(&mut commands, &mut game, &origin);
    }
}

/// Escape cancels as Cancel does: nothing else on the card takes it.
fn cancel_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    origin: Option<Res<SessionOrigin>>,
    mut game: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if let Some(origin) = origin {
        cancel(&mut commands, &mut game, &origin);
    }
}

fn cancel(commands: &mut Commands, game: &mut NextState<GameState>, origin: &SessionOrigin) {
    if end_session(commands, game, origin, None) {
        info!("Connection to {:?} cancelled", origin.name());
    }
}
