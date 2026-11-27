//! The hints of the first minutes of a new player in a survival world, at the top left of the
//! game: one at a time, each fading once done, or after a while. The server keeps which are done
//! (`Progress::steps`), so that none comes back.

use crate::i18n::tr;
use bevy::prelude::*;
use bevy_renet::RenetClient;
use shared::messages::ClientToServerMessage;
use shared::players::game_mode::GameMode;
use shared::players::progress::{Achievement, FirstStep, Progress};
use shared::players::Player;

use super::corner::TopLeftColumn;
use super::progress::PlayerProgress;
use super::{InventoryRoot, UIMode};
use crate::camera::CameraController;
use crate::input::data::GameAction;
use crate::network::SendGameMessageExtension;
use crate::player::CurrentPlayerMarker;
use crate::state::{GameSet, GameState, KeyMap};
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::menus::pause::PauseMenu;
use crate::ui::menus::settings::controls::key_label;
use crate::ui::settings::{HintsChoice, InterfaceSettings};
use crate::ui::theme;
use crate::ui::widgets::hints::key_cap;

const FADE_SECONDS: f32 = 0.6;
/// Clear of the minimap at the right of a window of 800 px.
const WIDTH: f32 = 340.0;
const PANEL_ALPHA: f32 = 0.8;
/// How far a player walks and how much it turns to have learnt to, in blocks and radians.
const WALKED: f32 = 4.0;
const TURNED: f32 = 1.0;

/// A part of a hint: the caps of the keys of some actions, or a text.
/// The texts are keys of the language files.
enum Part {
    Keys(&'static [GameAction]),
    Cap(&'static str),
    Text(&'static str),
}

fn parts(step: FirstStep) -> &'static [Part] {
    use Part::*;
    match step {
        FirstStep::Move => &[
            Keys(&[
                GameAction::MoveForward,
                GameAction::MoveLeft,
                GameAction::MoveBackward,
                GameAction::MoveRight,
            ]),
            Text("hint.first.walk"),
        ],
        FirstStep::Wood => &[Cap("hint.first.left_click"), Text("hint.first.wood")],
        FirstStep::Inventory => &[
            Keys(&[GameAction::ToggleInventory]),
            Text("hint.first.inventory"),
        ],
        FirstStep::CraftingTable => &[
            Keys(&[GameAction::ToggleInventory]),
            Text("hint.first.planks"),
        ],
        FirstStep::Torch => &[Cap("hint.first.right_click"), Text("hint.first.torches")],
        FirstStep::Achievements => &[Keys(&[GameAction::Escape]), Text("hint.first.progress")],
    }
}

/// How long a hint shows when it is not followed.
fn seconds(step: FirstStep) -> f32 {
    match step {
        FirstStep::Move => 60.0,
        FirstStep::Inventory => 90.0,
        FirstStep::Wood | FirstStep::CraftingTable => 240.0,
        FirstStep::Torch => 300.0,
        FirstStep::Achievements => 15.0,
    }
}

/// The first step not done yet. A crafting table is made however it was got.
fn next_step(progress: &Progress, done: &[FirstStep]) -> Option<FirstStep> {
    FirstStep::ALL.into_iter().find(|step| {
        !progress.steps.contains(step)
            && !done.contains(step)
            && (*step != FirstStep::CraftingTable || !progress.has(Achievement::CraftingTable))
    })
}

/// What the player did of the steps the server cannot see, and how long the current hint showed.
#[derive(Resource, Default)]
struct FirstSteps {
    /// Told to the server, which has not answered yet.
    done: Vec<FirstStep>,
    shown: Option<FirstStep>,
    age: f32,
    walked: f32,
    turned: f32,
    last: Option<(Vec3, Quat)>,
}

/// The hint of a step.
#[derive(Component)]
pub struct FirstStepHint {
    pub step: FirstStep,
    opacity: f32,
}

pub fn first_steps_plugin(app: &mut App) {
    app.init_resource::<FirstSteps>()
        .add_systems(OnEnter(GameState::Game), |mut steps: ResMut<FirstSteps>| {
            *steps = default();
        })
        .add_systems(
            Update,
            (follow_steps, show_step, fade_hints)
                .chain()
                .after(GameSet::Reconciliation)
                .run_if(in_state(GameState::Game)),
        );
}

/// Follows what the player does, and tells the server the steps it did or left.
#[allow(clippy::too_many_arguments)]
fn follow_steps(
    mut steps: ResMut<FirstSteps>,
    progress: Res<PlayerProgress>,
    settings: Res<InterfaceSettings>,
    time: Res<Time>,
    ui_mode: Res<UIMode>,
    players: Query<&Player, With<CurrentPlayerMarker>>,
    cameras: Query<&Transform, With<CameraController>>,
    inventory: Query<&Visibility, With<InventoryRoot>>,
    pause: Query<(), With<PauseMenu>>,
    mut client: ResMut<RenetClient>,
) {
    let (Some(progress), Ok(player)) = (&progress.0, players.single()) else {
        return;
    };
    let mut did = Vec::new();
    if let Ok(camera) = cameras.single() {
        let now = (player.position, camera.rotation);
        if let Some((position, rotation)) = steps.last.replace(now) {
            steps.walked += position.distance(now.0).min(1.0);
            steps.turned += rotation.angle_between(now.1);
        }
        if steps.walked >= WALKED && steps.turned >= TURNED {
            did.push(FirstStep::Move);
        }
    }
    if inventory.iter().any(|v| *v == Visibility::Visible) {
        did.push(FirstStep::Inventory);
    }
    if steps.shown == Some(FirstStep::Achievements) && !pause.is_empty() {
        did.push(FirstStep::Achievements);
    }
    let shows = settings.hints == HintsChoice::On && player.game_mode == GameMode::Survival;
    let step = next_step(progress, &steps.done).filter(|_| shows);
    if steps.shown != step {
        steps.shown = step;
        steps.age = 0.0;
    }
    if let Some(step) = step {
        if *ui_mode == UIMode::Closed {
            steps.age += time.delta_secs();
        }
        if steps.age >= seconds(step) {
            did.push(step);
        }
    }
    // In creative, or with the hints off, nothing is learnt
    for step in did.into_iter().filter(|_| shows) {
        if !progress.steps.contains(&step) && !steps.done.contains(&step) {
            steps.done.push(step);
            client.send_game_message(ClientToServerMessage::FirstStep(step));
        }
    }
}

/// Spawns the hint of the current step, once the one before has faded.
fn show_step(
    steps: Res<FirstSteps>,
    hints: Query<(Entity, &FirstStepHint)>,
    ui_mode: Res<UIMode>,
    key_map: Res<KeyMap>,
    assets: Res<AssetServer>,
    corner: Single<Entity, With<TopLeftColumn>>,
    mut commands: Commands,
) {
    if key_map.is_changed() {
        for (hint, _) in &hints {
            commands.entity(hint).despawn();
        }
    } else if !hints.is_empty() {
        return;
    }
    let Some(step) = steps.shown.filter(|_| *ui_mode == UIMode::Closed) else {
        return;
    };
    let font: Handle<Font> = assets.load(FONT_PATH);
    let glyph_font: Handle<Font> = assets.load(MONO_FONT_PATH);
    let hint = commands
        .spawn((
            FirstStepHint { step, opacity: 0.0 },
            Name::new(tr("hint.first.label")),
            // Last in the top-left column, under the debug overlay and the effects
            ChildOf(*corner),
            Node {
                width: px(WIDTH),
                padding: UiRect::all(px(theme::space_s())),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::space_xs()),
                ..default()
            },
            BackgroundColor(theme::panel().with_alpha(0.0)),
            Pickable::IGNORE,
            DespawnOnExit(GameState::Game),
        ))
        .id();
    let caps = commands
        .spawn((
            Node {
                column_gap: px(theme::space_xs()),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(hint),
        ))
        .id();
    let cap = |commands: &mut Commands, label: &str| {
        commands.spawn((
            key_cap(label, font.clone(), glyph_font.clone()),
            ChildOf(caps),
        ));
    };
    for part in parts(step) {
        match part {
            Part::Cap(label) => cap(&mut commands, tr(label)),
            Part::Keys(actions) => {
                for action in *actions {
                    let key = key_map.map.get(action).and_then(|keys| keys.first());
                    cap(
                        &mut commands,
                        &key.map_or(tr("hint.first.no_key").into(), |&k| key_label(k)),
                    );
                }
            }
            Part::Text(text) => {
                commands.spawn((
                    Text::new(tr(text)),
                    TextFont {
                        font: font.clone().into(),
                        font_size: px(theme::font_s()).into(),
                        ..default()
                    },
                    TextColor(theme::text()),
                    Pickable::IGNORE,
                    ChildOf(hint),
                ));
            }
        }
    }
}

/// Fades the hint in, and out once its step is done or a dialog opens.
fn fade_hints(
    steps: Res<FirstSteps>,
    ui_mode: Res<UIMode>,
    time: Res<Time>,
    mut hints: Query<(Entity, &mut FirstStepHint)>,
    children: Query<&Children>,
    mut texts: Query<&mut TextColor>,
    mut backgrounds: Query<&mut BackgroundColor>,
    mut borders: Query<&mut BorderColor>,
    mut commands: Commands,
) {
    for (entity, mut hint) in &mut hints {
        let stays = steps.shown == Some(hint.step) && *ui_mode == UIMode::Closed;
        let change = time.delta_secs() / FADE_SECONDS;
        let opacity = (hint.opacity + if stays { change } else { -change }).clamp(0.0, 1.0);
        if !stays && opacity == 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if opacity == hint.opacity {
            continue;
        }
        hint.opacity = opacity;
        if let Ok(mut background) = backgrounds.get_mut(entity) {
            background.0.set_alpha(opacity * PANEL_ALPHA);
        }
        for part in children.iter_descendants(entity) {
            if let Ok(mut text) = texts.get_mut(part) {
                text.0.set_alpha(opacity);
                continue;
            }
            // The row of the caps has no colours
            let Ok(mut background) = backgrounds.get_mut(part) else {
                continue;
            };
            if background.0 == Color::NONE {
                continue;
            }
            background.0.set_alpha(opacity);
            if let Ok(mut border) = borders.get_mut(part) {
                let border = &mut *border;
                let sides = [
                    &mut border.top,
                    &mut border.right,
                    &mut border.bottom,
                    &mut border.left,
                ];
                for side in sides {
                    side.set_alpha(opacity);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steps_come_in_order_and_none_twice() {
        let mut progress = Progress::default();
        assert_eq!(next_step(&progress, &[]), Some(FirstStep::Move));
        assert_eq!(
            next_step(&progress, &[FirstStep::Move]),
            Some(FirstStep::Wood)
        );
        progress.steps = vec![FirstStep::Move, FirstStep::Wood, FirstStep::Inventory];
        progress.achievements = vec![Achievement::CraftingTable];
        assert_eq!(next_step(&progress, &[]), Some(FirstStep::Torch));
        progress.steps = FirstStep::ALL.to_vec();
        assert_eq!(next_step(&progress, &[]), None);
    }
}
