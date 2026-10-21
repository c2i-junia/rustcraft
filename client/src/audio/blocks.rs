//! Block sounds: a hit while a block is mined, its break, a placement, the splash of a bucket.
//! The local player's changes are heard as it predicts them; the others' as the server announces
//! them.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use shared::world::{liquid::is_source, BlockData, BlockId};
use shared::HALF_BLOCK;

use super::playback::PlaySound;
use super::sounds::{Sound, SoundMaterial};
use crate::world::BlockChanged;

/// How long after the local player changed a block the server's updates of that block are
/// taken as the echo of that change, and not as another player's: the server confirms the
/// change, and may first restate the block as it was a few inputs earlier.
const ECHO_WINDOW_SECS: f32 = 1.0;

/// Shortest time between two mining sounds: the block takes a hit every `HIT_MS`, too often to
/// play each one.
const DIG_INTERVAL_SECS: f32 = 0.2;

/// The sound of a block going from `before` to `after`.
fn block_sound(
    before: Option<BlockData>,
    after: Option<BlockData>,
    by_local_player: bool,
) -> Option<Sound> {
    // The flow never makes nor takes a source: one that comes or goes amid water or air is a
    // bucket's
    let water_or_air = |block: Option<BlockData>| block.is_none_or(|b| b.id == BlockId::Water);
    let source = |block: Option<BlockData>| block.is_some_and(|b| is_source(&b));
    if water_or_air(before) && water_or_air(after) && source(before) != source(after) {
        return Some(Sound::Splash);
    }
    let (sound, block): (fn(SoundMaterial) -> Sound, _) = match (before, after) {
        (Some(before), None) => (Sound::Break, before.id),
        (Some(before), Some(after)) if before.id != after.id => (Sound::Place, after.id),
        // Only the local player's mining is heard: the server sends the others' progress
        // in bursts, at its tick rate
        (Some(before), Some(after))
            if by_local_player && after.breaking_progress > before.breaking_progress =>
        {
            (Sound::Dig, before.id)
        }
        // A block that reappears half mined is restated by the server, not placed
        (None, Some(after)) if after.breaking_progress == 0 => (Sound::Place, after.id),
        _ => return None,
    };
    SoundMaterial::of(block).map(sound)
}

pub fn block_sounds(
    mut changes: MessageReader<BlockChanged>,
    mut sounds: MessageWriter<PlaySound>,
    time: Res<Time>,
    // When the local player last changed each block
    mut local_changes: Local<HashMap<IVec3, f32>>,
    mut last_dig: Local<Option<f32>>,
) {
    let now = time.elapsed_secs();
    local_changes.retain(|_, at| now - *at < ECHO_WINDOW_SECS);

    for change in changes.read() {
        if change.by_local_player {
            local_changes.insert(change.position, now);
        } else if local_changes.contains_key(&change.position) {
            continue;
        }

        let Some(sound) = block_sound(change.before, change.after, change.by_local_player) else {
            continue;
        };
        if let Sound::Dig(_) = sound {
            if last_dig.is_some_and(|last| now - last < DIG_INTERVAL_SECS) {
                continue;
            }
            *last_dig = Some(now);
        }
        sounds.write(PlaySound::at(sound, change.position.as_vec3() + HALF_BLOCK));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::BlockDirection;

    fn block(id: BlockId, breaking_progress: u8) -> Option<BlockData> {
        Some(BlockData {
            breaking_progress,
            ..BlockData::new(id, BlockDirection::Front)
        })
    }

    #[test]
    fn a_removed_block_breaks_and_a_new_one_is_placed() {
        let stone = SoundMaterial::Stone;
        for by_local_player in [true, false] {
            let breaks = block_sound(block(BlockId::Cobblestone, 3), None, by_local_player);
            assert_eq!(breaks, Some(Sound::Break(stone)));
            let placed = block_sound(None, block(BlockId::Stone, 0), by_local_player);
            assert_eq!(placed, Some(Sound::Place(stone)));
        }
    }

    #[test]
    fn only_the_local_player_mining_is_heard() {
        let (before, after) = (block(BlockId::OakLog, 1), block(BlockId::OakLog, 2));
        let wood = SoundMaterial::Wood;
        assert_eq!(block_sound(before, after, true), Some(Sound::Dig(wood)));
        assert_eq!(block_sound(before, after, false), None);
    }

    #[test]
    fn a_block_restated_half_mined_is_not_placed() {
        assert_eq!(block_sound(None, block(BlockId::Dirt, 4), false), None);
    }

    #[test]
    fn flowing_water_makes_no_block_sound() {
        let flowing = |level| {
            Some(BlockData {
                level,
                ..BlockData::new(BlockId::Water, BlockDirection::Front)
            })
        };
        for local in [true, false] {
            assert_eq!(block_sound(flowing(3), None, local), None);
            assert_eq!(block_sound(None, flowing(1), local), None);
            assert_eq!(block_sound(flowing(1), flowing(2), local), None);
        }
    }

    #[test]
    fn a_source_taken_or_poured_by_a_bucket_splashes() {
        let source = block(BlockId::Water, 0);
        let flowing = Some(BlockData {
            level: 2,
            ..BlockData::new(BlockId::Water, BlockDirection::Front)
        });
        for local in [true, false] {
            assert_eq!(block_sound(source, None, local), Some(Sound::Splash));
            assert_eq!(block_sound(None, source, local), Some(Sound::Splash));
            assert_eq!(block_sound(flowing, source, local), Some(Sound::Splash));
            assert_eq!(block_sound(source, source, local), None);
        }
        // A block placed into a source sounds like that block
        let dirt = block(BlockId::Dirt, 0);
        assert_eq!(
            block_sound(source, dirt, true),
            Some(Sound::Place(SoundMaterial::Dirt))
        );
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<BlockChanged>()
            .add_message::<PlaySound>()
            .init_resource::<Time>()
            .add_systems(Update, block_sounds);
        app
    }

    fn heard(app: &mut App) -> Vec<Sound> {
        app.world_mut()
            .resource_mut::<Messages<PlaySound>>()
            .drain()
            .map(|request| request.sound)
            .collect()
    }

    fn change(before: Option<BlockData>, after: Option<BlockData>, local: bool) -> BlockChanged {
        BlockChanged {
            position: IVec3::new(1, 2, 3),
            before,
            after,
            by_local_player: local,
        }
    }

    #[test]
    fn the_server_echo_of_a_local_break_is_not_heard_again() {
        let mut app = app();
        let sand = block(BlockId::Sand, 4);
        app.world_mut().write_message(change(sand, None, true));
        app.update();
        assert_eq!(heard(&mut app), vec![Sound::Break(SoundMaterial::Sand)]);

        // The server restates the block one input before it broke, then confirms the break
        app.world_mut().write_message(change(None, sand, false));
        app.world_mut().write_message(change(sand, None, false));
        app.update();
        assert!(heard(&mut app).is_empty());
    }

    #[test]
    fn mining_hits_are_spaced_out() {
        let mut app = app();
        for progress in 0..5 {
            let before = block(BlockId::Stone, progress);
            let after = block(BlockId::Stone, progress + 1);
            app.world_mut().write_message(change(before, after, true));
        }
        app.update();
        assert_eq!(heard(&mut app), vec![Sound::Dig(SoundMaterial::Stone)]);
    }
}
