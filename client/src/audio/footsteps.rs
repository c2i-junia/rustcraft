//! Footsteps on the block under the feet, and the thud of a landing.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use shared::messages::PlayerId;
use shared::players::{MovementState, Player, PLAYER_HEIGHT};
use shared::world::{BlockHitbox, BlockId, ChunkMap};

use super::playback::PlaySound;
use super::sounds::{Sound, SoundMaterial};
use crate::player::{CurrentPlayerMarker, LocalPlayerMoved};
use crate::world::ClientWorldMap;

/// Distance walked between two steps, in blocks: about 3 steps per second at walking speed.
const STRIDE: f32 = 1.8;

/// How far under the feet a block is still stood on.
const GROUND_PROBE: f32 = 0.1;

/// Falling speed, in blocks per second, from which touching the ground is a landing: a jump
/// lands at about 8.7, a fall from one block at about 7.7.
const LANDING_SPEED: f32 = 7.0;

/// The block a player whose box is centred on `position` stands on, if any.
fn ground_under(position: Vec3, height: f32, blocks: &ChunkMap) -> Option<BlockId> {
    let below_feet = position - Vec3::Y * (height / 2.0 + GROUND_PROBE);
    let block = blocks.get_block(below_feet.floor().as_ivec3())?;
    matches!(block.id.get_hitbox(), BlockHitbox::FullBlock).then_some(block.id)
}

/// The distance a player walked since its last step.
#[derive(Default)]
pub struct Stride(f32);

impl Stride {
    /// Walks from `from` to `to` on `ground`, and returns the step heard, if one is.
    fn walk(&mut self, from: Vec3, to: Vec3, ground: Option<BlockId>) -> Option<Sound> {
        let material = SoundMaterial::of(ground?)?;
        self.0 += from.xz().distance(to.xz());
        if self.0 < STRIDE {
            return None;
        }
        self.0 %= STRIDE;
        Some(Sound::Step(material))
    }
}

/// The sound of one frame of the local player's movement. A frame that goes through a portal is
/// heard as such, and its leap from a portal to the other is not walked.
fn local_movement_sound(
    stride: &mut Stride,
    from: MovementState,
    to: MovementState,
    through_portal: bool,
    blocks: &ChunkMap,
) -> Option<Sound> {
    if through_portal {
        return Some(Sound::PortalPass);
    }
    if to.is_flying {
        return None;
    }
    if to.on_ground && from.velocity.y <= -LANDING_SPEED {
        return Some(Sound::Land);
    }
    let ground = ground_under(to.position, PLAYER_HEIGHT, blocks);
    stride.walk(from.position, to.position, ground)
}

pub fn local_footsteps(
    mut moves: MessageReader<LocalPlayerMoved>,
    world_map: Res<ClientWorldMap>,
    mut stride: Local<Stride>,
    mut sounds: MessageWriter<PlaySound>,
) {
    for moved in moves.read() {
        let sound = local_movement_sound(
            &mut stride,
            moved.from,
            moved.to,
            moved.through_portal,
            &world_map.blocks,
        );
        if let Some(sound) = sound {
            sounds.write(PlaySound::here(sound));
        }
    }
}

/// How far another player may move between two of its states and still have walked: farther, it
/// went through a portal, respawned or was teleported.
const MAX_REMOTE_WALK: f32 = 2.0;

/// The step heard of another player seen moving from `from` to `to` on `ground`, if one is. A
/// leap is not walked.
fn remote_step(
    stride: &mut Stride,
    from: Vec3,
    to: Vec3,
    ground: Option<BlockId>,
) -> Option<Sound> {
    if from.distance(to) > MAX_REMOTE_WALK {
        return None;
    }
    stride.walk(from, to, ground)
}

/// The steps of the other players, guessed from their positions: the server sends nothing
/// else about them.
pub fn other_players_footsteps(
    players: Query<&Player, Without<CurrentPlayerMarker>>,
    world_map: Res<ClientWorldMap>,
    // Where each player was last seen, and how far it walked since its last step
    mut tracked: Local<HashMap<PlayerId, (Vec3, Stride)>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    tracked.retain(|id, _| players.iter().any(|player| player.id == *id));
    for player in &players {
        let (last_position, stride) = tracked
            .entry(player.id)
            .or_insert_with(|| (player.position, Stride::default()));
        if *last_position == player.position {
            continue;
        }
        let ground = ground_under(player.position, player.height, &world_map.blocks);
        if let Some(sound) = remote_step(stride, *last_position, player.position, ground) {
            let feet = player.position - Vec3::Y * player.height / 2.0;
            sounds.write(PlaySound::at(sound, feet));
        }
        *last_position = player.position;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::messages::{NetworkAction, PlayerFrameInput};
    use shared::players::movement::simulate_player_movement;

    /// Top surface of the test floor, made of `block`.
    const FLOOR_TOP: f32 = 64.0;

    fn floor(block: BlockId) -> ChunkMap {
        let mut world =
            ChunkMap::with_loaded_region(IVec3::new(-40, 48, -40), IVec3::new(40, 95, 40));
        world.fill_box(IVec3::new(-40, 60, -40), IVec3::new(40, 63, 40), block);
        world
    }

    /// The sounds of `frames` frames of 16 ms from standing on the floor, with the actions
    /// `actions(frame)` held at each.
    fn sounds_of(
        world: &ChunkMap,
        frames: u32,
        actions: impl Fn(u32) -> Vec<NetworkAction>,
    ) -> Vec<Sound> {
        let mut player = Player::new(
            1,
            "tester".into(),
            Vec3::new(0.5, FLOOR_TOP + PLAYER_HEIGHT / 2.0, 0.5),
            Transform::default(),
        );
        // in creative, which may fly
        player.game_mode = shared::players::game_mode::GameMode::Creative;
        let mut stride = Stride::default();
        (0..frames)
            .filter_map(|frame| {
                let input = PlayerFrameInput {
                    delta_ms: 16,
                    inputs: actions(frame).into_iter().collect(),
                    ..default()
                };
                let from = player.movement_state();
                let outcome = simulate_player_movement(&mut player, world, &input);
                let through_portal = outcome.turned.is_some();
                local_movement_sound(
                    &mut stride,
                    from,
                    player.movement_state(),
                    through_portal,
                    world,
                )
            })
            .collect()
    }

    #[test]
    fn walking_steps_once_per_stride_on_the_floor_material() {
        // 50 frames at 5 blocks per second: 4 blocks, two strides
        let steps = sounds_of(&floor(BlockId::Sand), 50, |_| {
            vec![NetworkAction::MoveForward]
        });
        assert_eq!(steps, vec![Sound::Step(SoundMaterial::Sand); 2]);
    }

    #[test]
    fn standing_still_makes_no_sound() {
        assert!(sounds_of(&floor(BlockId::Stone), 200, |_| vec![]).is_empty());
    }

    #[test]
    fn a_jump_lands_once() {
        let jump_once = |frame| match frame {
            0 => vec![NetworkAction::JumpOrFlyUp],
            _ => vec![],
        };
        assert_eq!(
            sounds_of(&floor(BlockId::Grass), 200, jump_once),
            vec![Sound::Land]
        );
    }

    #[test]
    fn flying_makes_no_sound() {
        let fly_forward = |frame| match frame {
            0 => vec![NetworkAction::ToggleFlyMode],
            _ => vec![NetworkAction::MoveForward],
        };
        assert!(sounds_of(&floor(BlockId::Stone), 100, fly_forward).is_empty());
    }

    #[test]
    fn going_through_a_portal_is_heard_instead_of_a_step() {
        use shared::world::{FaceDirection, Portal, PortalColor};
        let mut world = floor(BlockId::Sand);
        // The blue portal faces the player, 3.5 blocks ahead; the orange one is 20 blocks away
        // and faces the same way, so that the player walks on out of it
        for (color, anchor, face) in [
            (
                PortalColor::Blue,
                IVec3::new(-1, 64, -4),
                FaceDirection::PlusZ,
            ),
            (
                PortalColor::Orange,
                IVec3::new(20, 64, 10),
                FaceDirection::MinusZ,
            ),
        ] {
            world.portals_mut().insert(Portal {
                owner: 1,
                color,
                anchor,
                face,
            });
        }
        // 100 frames at 5 blocks per second: 8 blocks, the crossing on the way
        let sounds = sounds_of(&world, 100, |_| vec![NetworkAction::MoveForward]);
        let passes = sounds.iter().filter(|s| **s == Sound::PortalPass).count();
        assert_eq!(passes, 1, "{sounds:?}");
        // The steps of the 8 blocks walked, not of the jump between the portals
        let steps = sounds
            .iter()
            .filter(|s| **s == Sound::Step(SoundMaterial::Sand))
            .count();
        assert_eq!(steps, 4, "{sounds:?}");
    }

    #[test]
    fn another_player_that_leaps_through_a_portal_or_respawns_takes_no_step() {
        let mut stride = Stride::default();
        let stone = Some(BlockId::Stone);
        // Walked up to just short of a step
        assert_eq!(
            remote_step(&mut stride, Vec3::ZERO, Vec3::X * 1.7, stone),
            None
        );
        // Then out of a portal 20 blocks away
        let out = Vec3::new(20.0, 0.0, 5.0);
        assert_eq!(remote_step(&mut stride, Vec3::X * 1.7, out, stone), None);
        // And on: the stride it had is kept
        assert_eq!(
            remote_step(&mut stride, out, out + Vec3::X * 0.2, stone),
            Some(Sound::Step(SoundMaterial::Stone))
        );
    }

    #[test]
    fn flowers_are_not_stood_on() {
        let mut world = floor(BlockId::Dirt);
        world.fill_box(IVec3::new(0, 64, 0), IVec3::new(0, 64, 0), BlockId::Poppy);
        let feet_in_the_flower = Vec3::new(0.5, 64.5 + PLAYER_HEIGHT / 2.0, 0.5);
        assert_eq!(
            ground_under(feet_in_the_flower, PLAYER_HEIGHT, &world),
            None
        );
        let on_the_dirt = Vec3::new(0.5, FLOOR_TOP + PLAYER_HEIGHT / 2.0, 0.5);
        assert_eq!(
            ground_under(on_the_dirt, PLAYER_HEIGHT, &world),
            Some(BlockId::Dirt)
        );
    }
}
