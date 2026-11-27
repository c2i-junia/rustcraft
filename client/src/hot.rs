//! Dev-only support of hot patching, compiled with the `hot` feature (`just hot`, see
//! docs/HOT_RELOAD.md):
//! - a patch re-enters the screen shown, so that a change to its setup function is seen without
//!   leaving it: the menu screen, or in game the HUD and the pause menu;
//! - a system a patch could not reach is named in a warning: its code stays the one the game
//!   was started with, which is what a change to its parameters does.

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use bevy::app::{FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, RunFixedMainLoop};
use bevy::ecs::schedule::ScheduleLabel;
use bevy::ecs::HotPatched;
use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Event, Subscriber};
use bevy::log::tracing_subscriber::{layer::Context, Layer};
use bevy::log::BoxedLayer;
use bevy::prelude::*;

use crate::network::leave::leaving;
use crate::network::session::SessionOrigin;
use crate::state::GameState;
use crate::ui::hud::minimap::world_map::WorldMapRoot;
use crate::ui::hud::{respawn_hud, UiDialog};
use crate::ui::menus::death::DeathScreen;
use crate::ui::menus::pause::{spawn_pause_menu, PauseMenu};
use crate::ui::menus::progress::ProgressRoot;
use crate::ui::menus::settings::SettingsRoot;
use crate::ui::menus::{modal_open, MenuState};
use crate::ui::quit::QuittingCard;
use crate::ui::widgets::text_field::text_field_focused;

pub fn hot_plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            note_patch,
            reenter_menu
                .run_if(resource_exists::<PatchPending>)
                .run_if(not(in_state(GameState::Game)))
                .run_if(not(modal_open)),
            rebuild_in_game_screens
                .run_if(resource_exists::<PatchPending>)
                .run_if(in_state(GameState::Game))
                .run_if(in_game_screens_at_rest)
                .run_if(not(modal_open))
                .run_if(not(text_field_focused))
                .run_if(not(leaving)),
        )
            .chain(),
    )
    .add_systems(First, warn_of_stale_systems);
}

/// A patch arrived whose screen has not been re-entered yet.
#[derive(Resource)]
struct PatchPending;

/// The frames left before the systems a patch reached are counted: those of `FixedUpdate` may
/// not run in the frame of the patch.
#[derive(Resource)]
struct StaleCheckIn(u32);

const STALE_CHECK_FRAMES: u32 = 10;

fn note_patch(mut patches: MessageReader<HotPatched>, mut commands: Commands) {
    if patches.read().count() > 0 {
        info!("hot patch applied");
        commands.insert_resource(PatchPending);
        commands.insert_resource(StaleCheckIn(STALE_CHECK_FRAMES));
    }
}

/// Re-enters the menu screen shown: its `OnExit` despawns it (`DespawnOnExit`) and its `OnEnter`
/// spawns it again, with the patched code. `NextState::set` runs both even for the same state.
/// A screen under a dialog waits for the dialog to close (`modal_open`): the dialog is not part
/// of the screen and would be left over the new one. While a world loads (`MenuState::Disabled`)
/// there is no screen to re-enter; in game, `rebuild_in_game_screens` rebuilds the HUD instead.
fn reenter_menu(
    menu: Res<State<MenuState>>,
    mut next_menu: ResMut<NextState<MenuState>>,
    mut commands: Commands,
) {
    commands.remove_resource::<PatchPending>();
    match menu.get() {
        MenuState::Disabled => info!("hot patch: no menu screen to re-enter"),
        &screen => {
            info!("hot patch: re-entering {screen:?}");
            next_menu.set(screen);
        }
    }
}

/// Whether nothing is open over the game but the pause menu: no inventory, chest or other
/// panel, no settings, progress, world map, death screen or quitting card. These are not rebuilt,
/// and what they show lives in the HUD (the inventory) or would be left over the new HUD.
fn in_game_screens_at_rest(
    dialogs: Query<&Visibility, (With<UiDialog>, Without<PauseMenu>)>,
    overlays: Query<
        (),
        Or<(
            With<SettingsRoot>,
            With<ProgressRoot>,
            With<WorldMapRoot>,
            With<DeathScreen>,
            With<QuittingCard>,
        )>,
    >,
) -> bool {
    overlays.is_empty() && dialogs.iter().all(|v| *v != Visibility::Visible)
}

/// Rebuilds the in-game screens with the patched code, without leaving the world: the HUD
/// (`respawn_hud`), then the pause menu if it is open. The chat is left as it is, with its lines.
/// The rebuild waits for the screens over the game to close (`in_game_screens_at_rest`) and for the
/// player to stop typing.
fn rebuild_in_game_screens(world: &mut World) {
    world.remove_resource::<PatchPending>();
    info!("hot patch: rebuilding the HUD");
    respawn_hud(world);
    world.run_system_cached(respawn_pause_menu).ok();
}

/// Spawns the pause menu again if it is open.
fn respawn_pause_menu(
    menus: Query<Entity, With<PauseMenu>>,
    origin: Option<Res<SessionOrigin>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    for menu in &menus {
        info!("hot patch: rebuilding the pause menu");
        commands.entity(menu).despawn();
        spawn_pause_menu(&mut commands, &assets, origin.as_deref());
    }
}

/// The systems Bevy moved to the patched code since the last check, by name: Bevy logs
/// `system <name> hotpatched` (debug, `bevy_ecs::system`) for each, and `PatchedSystems` reads it.
/// Shared by a resource rather than a `static`: the patched code would read its own copy.
#[derive(Resource, Clone, Default)]
struct Patched(Arc<Mutex<BTreeSet<String>>>);

/// The log layer that collects the systems Bevy moved to the patched code (`LogPlugin`'s
/// `custom_layer`). It sees nothing unless the log filter keeps `bevy_ecs::system=debug`.
pub fn patched_systems_layer(app: &mut App) -> Option<BoxedLayer> {
    let patched = Patched::default();
    app.insert_resource(patched.clone());
    Some(Box::new(PatchedSystems(patched)))
}

struct PatchedSystems(Patched);

impl<S: Subscriber> Layer<S> for PatchedSystems {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        struct Message(Option<String>);
        impl Visit for Message {
            fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
                if field.name() == "message" {
                    self.0 = Some(format!("{value:?}"));
                }
            }
        }
        let mut message = Message(None);
        event.record(&mut message);
        if let Some(name) = message
            .0
            .as_deref()
            .and_then(|m| m.strip_prefix("system "))
            .and_then(|m| m.strip_suffix(" hotpatched"))
        {
            self.0 .0.lock().unwrap().insert(name.to_string());
        }
    }
}

/// A few frames after a patch that rebuilt the client, warns of the systems of the client that
/// run every frame but were not moved to the patched code. Bevy finds a system's code in the
/// patch by the symbol of its `run`, which is generic over its parameters: a system whose
/// parameters changed has no such symbol in the patch and silently keeps its old code. Only the
/// schedules of every frame are checked (not `OnEnter`, which ran before the patch), and not
/// the one running the check (`First`), which `Schedules` lends out while it runs. A patch of
/// `shared` alone leaves the client's systems as they are: nothing is checked then.
fn warn_of_stale_systems(world: &mut World) {
    let Some(mut check) = world.get_resource_mut::<StaleCheckIn>() else {
        return;
    };
    if check.0 > 0 {
        check.0 -= 1;
        return;
    }
    world.remove_resource::<StaleCheckIn>();
    let patched = std::mem::take(&mut *world.resource::<Patched>().0.lock().unwrap());
    if patched.is_empty() {
        info!("hot patch: no system moved to the patched code (a patch of another crate, or the log filter hides `bevy_ecs::system=debug`)");
        return;
    }
    // `MainScheduleOrder` is out of the world while `Main` runs: the labels are those it holds
    let labels = [
        PreUpdate.intern(),
        StateTransition.intern(),
        RunFixedMainLoop.intern(),
        FixedFirst.intern(),
        FixedPreUpdate.intern(),
        FixedUpdate.intern(),
        FixedPostUpdate.intern(),
        FixedLast.intern(),
        Update.intern(),
        PostUpdate.intern(),
        Last.intern(),
    ];
    let schedules = world.resource::<Schedules>();
    let mut stale = BTreeSet::new();
    for label in labels {
        let Some(Ok(systems)) = schedules.get(label).map(Schedule::systems) else {
            continue;
        };
        for (_, system) in systems {
            let name = system.name().to_string();
            if name.starts_with("client::") && !patched.contains(&name) {
                stale.insert(name);
            }
        }
    }
    if stale.is_empty() {
        info!(
            "hot patch: {} systems moved to the patched code",
            patched.len()
        );
    } else {
        for name in &stale {
            warn!("hot patch: {name} still runs its OLD code: its parameters changed? Restart `just hot`");
        }
    }
}
