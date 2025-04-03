use bevy::prelude::*;

use crate::GameState;

// This plugin will display a splash screen with Bevy logo for 1 second before switching to the menu
pub fn splash_plugin(app: &mut App) {
    // As this plugin is managing the splash screen, it will focus on the state `GameState::Splash`
    app
        // When entering the state, spawn everything needed for this screen
        .add_systems(OnEnter(GameState::Splash), splash_setup)
        // While in this state, run the `countdown` system
        .add_systems(Update, countdown.run_if(in_state(GameState::Splash)));
}

// Newtype to use a `Timer` for this screen as a resource
#[derive(Resource, Deref, DerefMut)]
struct SplashTimer(Timer);

fn splash_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Display the logo
    let icon = asset_server.load("./graphics/bevy_icon.png");
    // debug!("splash screen");

    commands
        .spawn((
            (Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            }),
            StateScoped(GameState::Splash),
        ))
        .with_children(|parent| {
            parent.spawn((
                Node {
                    // This will set the logo to be 200px wide, and auto adjust its height
                    width: Val::Px(200.0),
                    ..default()
                },
                ImageNode::new(icon),
            ));
        });
    // Insert the timer as a resource
    commands.insert_resource(SplashTimer(Timer::from_seconds(1.0, TimerMode::Once)));
}

// Tick the timer, and change state when finished
fn countdown(
    mut game_state: ResMut<NextState<GameState>>,
    time: Res<Time>,
    mut timer: ResMut<SplashTimer>,
) {
    if timer.tick(time.delta()).finished() {
        game_state.set(GameState::Menu);
    }
}
