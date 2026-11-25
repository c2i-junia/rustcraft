//! The credits (`data/credits.txt`), which scroll up over the whole window until their end, or
//! until the player skips them. They show over the game once the Ember Mother is beaten; any
//! other screen may spawn them (`spawn_credits`).

use bevy::input_focus::InputFocus;
use bevy::prelude::*;

use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::hud::UiDialog;
use crate::ui::theme::{ACCENT, FONT_L, FONT_M, FONT_XL, MUTED, SPACE_L, SPACE_S, TEXT};

const CREDITS: &str = include_str!("../../../data/credits.txt");

/// How fast they scroll, in logical px a second, and faster while Space is held.
const SPEED: f32 = 42.0;
const FAST: f32 = 4.0;
/// How long the last lines stay.
const HOLD_SECONDS: f32 = 3.0;
pub const SKIP_HINT: &str = "Esc  Skip";

/// A line of the credits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsLine {
    Title(&'static str),
    Text(&'static str),
    Space,
}

/// The lines of the credits, from the first to the last.
pub fn lines() -> impl Iterator<Item = CreditsLine> {
    CREDITS.lines().map(|line| match line.strip_prefix("# ") {
        Some(title) => CreditsLine::Title(title),
        None if line.trim().is_empty() => CreditsLine::Space,
        None => CreditsLine::Text(line),
    })
}

/// The credits showing, scrolled for `age` seconds so far.
#[derive(Component, Default)]
pub struct Credits {
    age: f32,
    held: f32,
}

#[derive(Component)]
struct CreditsRoll;

/// The boss was beaten, and the credits have to show.
#[derive(Resource, Debug, Default)]
pub struct CreditsDue(pub bool);

pub fn credits_plugin(app: &mut App) {
    app.init_resource::<CreditsDue>()
        .add_systems(
            Update,
            (
                show_credits_due.run_if(in_state(GameState::Game)),
                scroll_credits,
            )
                .chain()
                .after(crate::state::GameSet::UiMode),
        )
        .add_systems(OnExit(GameState::Game), |mut due: ResMut<CreditsDue>| {
            due.0 = false;
        });
}

fn show_credits_due(
    mut due: ResMut<CreditsDue>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut focus: ResMut<InputFocus>,
    showing: Query<(), With<Credits>>,
) {
    if due.0 {
        due.0 = false;
        if showing.is_empty() {
            focus.clear();
            let root = spawn_credits(&mut commands, &assets);
            commands.entity(root).insert(DespawnOnExit(GameState::Game));
        }
    }
}

/// Spawns the credits over the window. They go by themselves, or when the player skips them.
pub fn spawn_credits(commands: &mut Commands, assets: &AssetServer) -> Entity {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let text = |text: &str, size: f32, color: Color| {
        (
            Text::new(text),
            TextFont {
                font: font.clone().into(),
                font_size: px(size).into(),
                ..default()
            },
            TextColor(color),
            TextLayout::justify(Justify::Center),
            Pickable::IGNORE,
        )
    };
    let root = commands
        .spawn((
            Credits::default(),
            Name::new("Credits"),
            UiDialog,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.015, 0.01, 0.98)),
            Visibility::Visible,
            // Over the death screen and the pause menu
            GlobalZIndex(20),
        ))
        .id();
    let roll = commands
        .spawn((
            CreditsRoll,
            Node {
                position_type: PositionType::Absolute,
                top: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(SPACE_S),
                max_width: percent(100),
                ..default()
            },
            ChildOf(root),
            Pickable::IGNORE,
        ))
        .id();
    for (i, line) in lines().enumerate() {
        let mut entity = match line {
            CreditsLine::Title(title) if i == 0 => {
                commands.spawn(text(title, FONT_XL * 2.0, ACCENT))
            }
            CreditsLine::Title(title) => commands.spawn(text(title, FONT_L, ACCENT)),
            CreditsLine::Text(line) => commands.spawn(text(line, FONT_M, TEXT)),
            CreditsLine::Space => commands.spawn(Node {
                height: px(SPACE_L * 2.0),
                ..default()
            }),
        };
        entity.insert(ChildOf(roll));
    }
    commands.spawn((
        text(SKIP_HINT, FONT_M, MUTED),
        Node {
            position_type: PositionType::Absolute,
            right: px(SPACE_L),
            bottom: px(SPACE_L),
            ..default()
        },
        ChildOf(root),
    ));
    root
}

/// Scrolls the credits up, until their last line is in the middle of the window; they go a moment
/// later, or at Escape.
fn scroll_credits(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut credits: Query<(Entity, &mut Credits, &ComputedNode, &Children)>,
    mut rolls: Query<(&mut Node, &ComputedNode), With<CreditsRoll>>,
) {
    for (root, mut credits, window, children) in &mut credits {
        let skipped = keys.just_pressed(KeyCode::Escape) && credits.age > 0.0;
        let faster = if keys.pressed(KeyCode::Space) {
            FAST
        } else {
            1.0
        };
        credits.age += time.delta_secs() * faster;
        let height = window.size().y * window.inverse_scale_factor();
        let mut over = false;
        for child in children {
            let Ok((mut node, roll)) = rolls.get_mut(*child) else {
                continue;
            };
            let length = roll.size().y * roll.inverse_scale_factor();
            // Not laid out yet
            if height == 0.0 || length == 0.0 {
                continue;
            }
            let last = height / 2.0 - length;
            let top = (height - credits.age * SPEED).max(last);
            node.top = px(top);
            if top <= last {
                credits.held += time.delta_secs();
                over = credits.held > HOLD_SECONDS;
            }
        }
        if skipped || over {
            commands.entity(root).despawn();
        }
    }
}
