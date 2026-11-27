//! The credits (`data/credits.txt`), which scroll up over the whole window until their end, or
//! until the player skips them. They show over the game once the Ember Mother is beaten, with the
//! statistics of the player after their opening, and over the main menu from its Credits button
//! (`spawn_credits`), without them.

use crate::i18n::{tr_as, trf};
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use shared::players::progress::Stats;

use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::hud::progress::PlayerProgress;
use crate::ui::hud::UiDialog;
use crate::ui::menus::progress::stat_values;
use crate::ui::theme::{ACCENT, FONT_L, FONT_M, FONT_XL, MUTED, SPACE_L, SPACE_S, TEXT};

const CREDITS: &str = include_str!("../../../data/credits.txt");

/// How fast they scroll, in logical px a second, and faster while Space is held.
const SPEED: f32 = 42.0;
const FAST: f32 = 4.0;
/// How long the last lines stay.
const HOLD_SECONDS: f32 = 3.0;
/// The title over the statistics of the player.
pub const STATS_TITLE: &str = "credits.stats_title";
pub const SKIP_HINT: &str = "credits.skip";

/// A line of the credits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsLine {
    Title(&'static str),
    Text(&'static str),
    Space,
}

/// The statistic of this place in `Stats::LABELS`, kept up to date: the server tells the last of
/// them a moment after the boss fell.
#[derive(Component)]
struct CreditsStat(usize);

fn stat_line(progress: &PlayerProgress, i: usize) -> String {
    trf(
        "credits.stat",
        &[
            ("label", &tr_as("stat", Stats::LABELS[i])),
            ("value", &stat_values(progress)[i]),
        ],
    )
}

fn show_stats(progress: Res<PlayerProgress>, mut stats: Query<(&CreditsStat, &mut Text)>) {
    for (stat, mut text) in &mut stats {
        text.0 = stat_line(&progress, stat.0);
    }
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
pub struct CreditsRoll;

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
                show_stats.run_if(resource_changed::<PlayerProgress>),
            )
                .chain()
                .after(crate::state::GameSet::UiMode),
        )
        .add_systems(OnExit(GameState::Game), forget_credits);
}

/// Forgets the credits due in a session that ends.
pub fn forget_credits(mut due: ResMut<CreditsDue>) {
    due.0 = false;
}

fn show_credits_due(
    mut due: ResMut<CreditsDue>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut focus: ResMut<InputFocus>,
    progress: Res<PlayerProgress>,
    showing: Query<(), With<Credits>>,
) {
    if due.0 {
        due.0 = false;
        if showing.is_empty() {
            focus.clear();
            let root = spawn_credits(&mut commands, &assets, &progress);
            commands.entity(root).insert(DespawnOnExit(GameState::Game));
        }
    }
}

/// Spawns the credits over the window, with the statistics of the game `progress` is of, if any.
/// They go by themselves, or when the player skips them.
pub fn spawn_credits(
    commands: &mut Commands,
    assets: &AssetServer,
    progress: &PlayerProgress,
) -> Entity {
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
    let mut stats_due = progress.0.is_some();
    for (i, line) in lines().enumerate() {
        if line == CreditsLine::Space && std::mem::take(&mut stats_due) {
            let space = Node {
                height: px(SPACE_L * 2.0),
                ..default()
            };
            commands.spawn((space, ChildOf(roll)));
            commands.spawn((
                text(crate::i18n::tr(STATS_TITLE), FONT_L, ACCENT),
                ChildOf(roll),
            ));
            for stat in 0..Stats::LABELS.len() {
                let line = stat_line(progress, stat);
                commands.spawn((CreditsStat(stat), text(&line, FONT_M, TEXT), ChildOf(roll)));
            }
        }
        let mut entity = match line {
            CreditsLine::Title(title) if i == 0 => {
                commands.spawn(text(title, FONT_XL * 2.0, ACCENT))
            }
            CreditsLine::Title(title) => {
                commands.spawn(text(tr_as("credits", title), FONT_L, ACCENT))
            }
            CreditsLine::Text(line) => commands.spawn(text(line, FONT_M, TEXT)),
            CreditsLine::Space => commands.spawn(Node {
                height: px(SPACE_L * 2.0),
                ..default()
            }),
        };
        entity.insert(ChildOf(roll));
    }
    commands.spawn((
        text(crate::i18n::tr(SKIP_HINT), FONT_M, MUTED),
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
pub fn scroll_credits(
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
