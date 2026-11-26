//! The flames of a burning local player: two fires over the bottom corners of the screen, under
//! the rest of the HUD, cut frame by frame from the block texture of the fire (Pixel
//! Perfection's `fire_0.png`, 8 frames of 16x16).

use crate::i18n::tr;
use bevy::prelude::*;
use shared::players::Player;

use crate::player::CurrentPlayerMarker;
use crate::state::GameState;

pub const FIRE_PATH: &str = "./graphics/base_textures/blocks/Fire.png";

/// The frames of the strip, and how long each is shown: 2 ticks, as `Fire.anim.ron`.
const FRAMES: u32 = 8;
const FRAME_SECONDS: f32 = 0.1;
/// The side of a frame in the texture, in pixels.
const FRAME_SIZE: f32 = 16.0;

/// The flames over the screen.
#[derive(Component)]
pub struct BurningFlames;

/// A flame of the two.
#[derive(Component)]
pub struct Flame;

/// The frame of the fire shown `elapsed` seconds into the game.
pub fn flame_frame(elapsed: f32) -> u32 {
    (elapsed / FRAME_SECONDS) as u32 % FRAMES
}

/// The part of the strip that holds `frame`.
fn frame_rect(frame: u32) -> Rect {
    let top = frame as f32 * FRAME_SIZE;
    Rect::new(0.0, top, FRAME_SIZE, top + FRAME_SIZE)
}

/// Spawns the flames, hidden until the local player burns.
pub fn spawn_burning_flames(mut commands: Commands, assets: Res<AssetServer>) {
    let texture: Handle<Image> = assets.load(FIRE_PATH);
    let row = commands
        .spawn((
            BurningFlames,
            Name::new(tr("hud.burning")),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.),
                bottom: px(0.),
                width: percent(100),
                height: percent(45),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            // Under the hotbar and the hearts
            GlobalZIndex(-1),
            Visibility::Hidden,
            Pickable::IGNORE,
            DespawnOnExit(GameState::Game),
        ))
        .id();
    for flip_x in [false, true] {
        commands.spawn((
            Flame,
            ImageNode {
                image: texture.clone(),
                rect: Some(frame_rect(0)),
                flip_x,
                color: Color::WHITE.with_alpha(0.9),
                ..default()
            },
            Node {
                width: percent(40),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ));
    }
}

/// Shows the flames while the local player burns, on the frame of the moment.
pub fn update_burning_flames(
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut flames: Query<&mut Visibility, With<BurningFlames>>,
    mut images: Query<&mut ImageNode, With<Flame>>,
    time: Res<Time>,
) {
    let (Ok(player), Ok(mut visibility)) = (player.single(), flames.single_mut()) else {
        return;
    };
    if !player.is_burning() {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    }
    visibility.set_if_neq(Visibility::Inherited);
    let rect = Some(frame_rect(flame_frame(time.elapsed_secs())));
    for mut image in &mut images {
        if image.rect != rect {
            image.rect = rect;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flames_go_through_the_eight_frames_two_ticks_each() {
        assert_eq!(flame_frame(0.0), 0);
        assert_eq!(flame_frame(0.15), 1);
        assert_eq!(flame_frame(0.75), 7);
        assert_eq!(flame_frame(0.85), 0);
        assert_eq!(frame_rect(2), Rect::new(0.0, 32.0, 16.0, 48.0));
    }
}
