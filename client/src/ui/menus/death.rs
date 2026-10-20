//! The death screen, over the game while the local player is dead: what killed it, Respawn,
//! and the way out of the game that the pause menu has, which cannot open over it. The server
//! decides: the screen goes once the player it sends is alive again.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input_focus::{tab_navigation::TabGroup, AutoFocus, InputFocus},
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::{observe, Activate},
};
use bevy_renet::RenetClient;
use shared::messages::ClientToServerMessage;
use shared::players::Player;

use crate::network::session::SessionOrigin;
use crate::network::SendGameMessageExtension;
use crate::player::CurrentPlayerMarker;
use crate::state::{GameSet, GameState};
use crate::ui::assets::FONT_PATH;
use crate::ui::hud::{InventoryRoot, UiDialog};
use crate::ui::menus::pause::{act_on_pause_button, pause_buttons};
use crate::ui::theme::{FONT_L, FONT_XL, SPACE_L, SPACE_S, SPACE_XL, TEXT};
use crate::ui::widgets::button::{button, set_label, ButtonVariant};
use crate::ui::widgets::focus::{keep_focus_inside, ArrowNav};

pub const DEATH_TITLE: &str = "You died!";
pub const RESPAWN: &str = "Respawn";
/// The label of Respawn once clicked, until the server brings the player back.
pub const RESPAWNING: &str = "Respawning...";

/// The width of the buttons of the death screen.
const BUTTON_WIDTH: f32 = 320.0;

/// The root of the death screen.
#[derive(Component)]
pub struct DeathScreen;

/// The Respawn button.
#[derive(Component)]
pub struct RespawnButton;

pub fn death_plugin(app: &mut App) {
    app.add_systems(
        Update,
        show_death_screen
            .after(GameSet::Reconciliation)
            .run_if(in_state(GameState::Game)),
    );
}

/// Opens the death screen when the local player dies, and closes it once it is alive again.
fn show_death_screen(
    player: Query<&Player, With<CurrentPlayerMarker>>,
    screens: Query<Entity, With<DeathScreen>>,
    mut inventory: Query<&mut Visibility, With<InventoryRoot>>,
    origin: Option<Res<SessionOrigin>>,
    assets: Res<AssetServer>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let Ok(player) = player.single() else {
        return;
    };
    match (player.is_dead(), screens.single()) {
        (true, Err(_)) => {
            if let Ok(mut inventory) = inventory.single_mut() {
                *inventory = Visibility::Hidden;
            }
            let cause = player.death_cause.map(|cause| cause.message());
            spawn_death_screen(&mut commands, &assets, cause, origin.as_deref());
        }
        (false, Ok(screen)) => {
            commands.entity(screen).despawn();
            focus.clear();
        }
        _ => {}
    }
}

fn spawn_death_screen(
    commands: &mut Commands,
    assets: &AssetServer,
    cause: Option<&str>,
    origin: Option<&SessionOrigin>,
) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let root = commands
        .spawn((
            DeathScreen,
            UiDialog,
            AccessibilityNode(accesskit::Node::new(Role::Dialog)),
            AccessibleLabel::new(DEATH_TITLE),
            TabGroup::modal(),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(SPACE_L),
                ..default()
            },
            BackgroundColor(Color::srgba(0.35, 0.02, 0.0, 0.6)),
            Visibility::Visible,
            // under the pause menu, which a player dying with it open closes first
            GlobalZIndex(4),
            DespawnOnExit(GameState::Game),
        ))
        .id();
    commands.spawn((
        Text::new(DEATH_TITLE),
        TextFont {
            font: font.clone().into(),
            font_size: px(FONT_XL * 1.5).into(),
            ..default()
        },
        TextColor(TEXT),
        TextShadow::default(),
        ChildOf(root),
    ));
    if let Some(cause) = cause {
        commands.spawn((
            Text::new(cause),
            TextFont {
                font: font.clone().into(),
                font_size: px(FONT_L).into(),
                ..default()
            },
            TextColor(TEXT),
            TextShadow::default(),
            Node {
                margin: UiRect::bottom(px(SPACE_XL)),
                ..default()
            },
            ChildOf(root),
        ));
    }
    let column = commands
        .spawn((
            ArrowNav,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_S),
                width: px(BUTTON_WIDTH),
                max_width: percent(90),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let respawn = commands
        .spawn((
            button(RESPAWN, ButtonVariant::Primary, font.clone()),
            RespawnButton,
            AutoFocus,
            observe(respawn_on_click),
            ChildOf(column),
        ))
        .id();
    // the pause menu's way out of a world or a server: "Saving..." while leaving, like there
    if let Some(&leave) = pause_buttons(origin).iter().find(|action| action.leaves()) {
        commands.spawn((
            button(leave.label(), ButtonVariant::Default, font),
            leave,
            observe(act_on_pause_button),
            ChildOf(column),
        ));
    }
    keep_focus_inside(commands, root, Some(respawn));
}

/// Asks the server to bring the player back, once.
fn respawn_on_click(
    event: On<Activate>,
    mut client: ResMut<RenetClient>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut commands: Commands,
) {
    if disabled.contains(event.entity) {
        return;
    }
    client.send_game_message(ClientToServerMessage::Respawn);
    commands
        .entity(event.entity)
        .queue(set_label(RESPAWNING))
        .insert(InteractionDisabled);
}
