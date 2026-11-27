//! The screens shown, built again from the code and the data they are built from now, without
//! leaving them: after a hot patch (`hot.rs`) or a language file saved (`i18n/watch.rs`).
//! Anything can ask for it with `RebuildScreens`. In the menus the screen shown is entered again;
//! in game the HUD and the pause menu are spawned again, once nothing else is open over them.

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

pub fn rebuild_plugin(app: &mut App) {
    app.add_message::<RebuildScreens>().add_systems(
        Update,
        (
            note_rebuild,
            reenter_menu
                .run_if(resource_exists::<RebuildPending>)
                .run_if(not(in_state(GameState::Game)))
                .run_if(not(modal_open)),
            rebuild_in_game_screens
                .run_if(resource_exists::<RebuildPending>)
                .run_if(in_state(GameState::Game))
                .run_if(in_game_screens_at_rest)
                .run_if(not(modal_open))
                .run_if(not(text_field_focused))
                .run_if(not(leaving)),
        )
            .chain(),
    );
}

/// Asks for the screens shown to be built again.
#[derive(Message, Default)]
pub struct RebuildScreens;

/// A rebuild was asked for and is not done yet.
#[derive(Resource)]
struct RebuildPending;

fn note_rebuild(mut asked: MessageReader<RebuildScreens>, mut commands: Commands) {
    if asked.read().count() > 0 {
        commands.insert_resource(RebuildPending);
    }
}

/// Re-enters the menu screen shown: its `OnExit` despawns it (`DespawnOnExit`) and its `OnEnter`
/// spawns it again, with the code and texts of now. `NextState::set` runs both even for the same
/// state. A screen under a dialog waits for the dialog to close (`modal_open`): the dialog is not
/// part of the screen and would be left over the new one. While a world loads
/// (`MenuState::Disabled`) there is no screen to re-enter; in game, `rebuild_in_game_screens`
/// rebuilds the HUD instead.
fn reenter_menu(
    menu: Res<State<MenuState>>,
    mut next_menu: ResMut<NextState<MenuState>>,
    mut commands: Commands,
) {
    commands.remove_resource::<RebuildPending>();
    match menu.get() {
        MenuState::Disabled => info!("rebuild: no menu screen to re-enter"),
        &screen => {
            info!("rebuild: re-entering {screen:?}");
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

/// Rebuilds the in-game screens without leaving the world: the HUD (`respawn_hud`), then the
/// pause menu if it is open. The chat is left as it is, with its lines. The rebuild waits for the
/// screens over the game to close (`in_game_screens_at_rest`) and for the player to stop typing.
fn rebuild_in_game_screens(world: &mut World) {
    world.remove_resource::<RebuildPending>();
    info!("rebuild: rebuilding the HUD");
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
        info!("rebuild: rebuilding the pause menu");
        commands.entity(menu).despawn();
        spawn_pause_menu(&mut commands, &assets, origin.as_deref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                watch_for_changes_override: Some(false),
                ..default()
            },
        ))
        // what the pause menu loads, and what `text_field_focused` reads
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<bevy::input_focus::InputFocus>();
        app
    }

    fn at_rest(app: &mut App) -> bool {
        app.world_mut()
            .run_system_once(in_game_screens_at_rest)
            .unwrap()
    }

    #[test]
    fn the_screens_are_at_rest_with_only_the_pause_menu_or_hidden_panels_open() {
        let mut app = app();
        assert!(at_rest(&mut app));
        app.world_mut()
            .spawn((PauseMenu, UiDialog, Visibility::Visible));
        assert!(at_rest(&mut app), "the pause menu is rebuilt with the HUD");
        let panel = app.world_mut().spawn((UiDialog, Visibility::Hidden)).id();
        assert!(at_rest(&mut app), "a closed panel");
        app.world_mut()
            .entity_mut(panel)
            .insert(Visibility::Visible);
        assert!(!at_rest(&mut app), "an open panel, the inventory");
        app.world_mut().entity_mut(panel).despawn();
        app.world_mut().spawn(DeathScreen);
        assert!(!at_rest(&mut app), "the death screen");
    }

    #[derive(Resource, Default)]
    struct Entered(u32);

    #[test]
    fn a_rebuild_in_the_menus_enters_the_screen_shown_again() {
        let mut app = app();
        app.add_plugins((bevy::state::app::StatesPlugin, rebuild_plugin))
            .init_state::<GameState>()
            .insert_state(MenuState::Main)
            .init_resource::<Entered>()
            .add_systems(OnEnter(MenuState::Main), |mut e: ResMut<Entered>| e.0 += 1);
        app.update();
        assert_eq!(app.world().resource::<Entered>().0, 1);
        app.update();
        assert_eq!(app.world().resource::<Entered>().0, 1, "no rebuild asked");
        app.world_mut().write_message(RebuildScreens);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Entered>().0, 2);
        assert!(!app.world().contains_resource::<RebuildPending>());
    }

    fn pause_menus(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<PauseMenu>>()
            .iter(app.world())
            .collect()
    }

    #[test]
    fn an_open_pause_menu_is_spawned_again_and_a_closed_one_is_not() {
        let mut app = app();
        app.world_mut().run_system_once(respawn_pause_menu).unwrap();
        assert!(pause_menus(&mut app).is_empty());
        let old = app.world_mut().spawn(PauseMenu).id();
        app.world_mut().run_system_once(respawn_pause_menu).unwrap();
        let menus = pause_menus(&mut app);
        assert_eq!(menus.len(), 1);
        assert_ne!(menus[0], old);
    }
}
