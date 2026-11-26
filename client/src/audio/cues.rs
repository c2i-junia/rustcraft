//! The sounds of the later features, each heard from what the server already tells: armour put
//! on, taken off or broken, a bow drawn, an arrow ending its flight or picked up, an orb and a
//! level, a trade, the hearts, a birth and a taming, a mob falling into water, and the stings
//! of the progress (a depth reached, an achievement, a boss, the victory, the credits). What a
//! player or a mob does is heard where it is, by every player near; none plays more than once
//! a moment.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use shared::messages::PlayerId;
use shared::players::armour::WornArmour;
use shared::players::Player;
use shared::players::bow::CROSSBOW_LOAD_MS;
use shared::world::{BlockId, ItemId, MobId, MobKind};

use super::creatures::HEARING_DISTANCE;
use super::playback::PlaySound;
use super::sounds::{CreatureCall, Sound};
use crate::dropped_items::ItemPickedUp;
use crate::mob::{Mob, MobEntities};
use crate::player::armour::Worn;
use crate::player::hand::HeldItem;
use crate::player::CurrentPlayerMarker;
use crate::portal::PortalCamera;
use crate::state::GameState;
use crate::ui::credits::Credits;
use crate::ui::hud::boss::NearBoss;
use crate::ui::hud::depths::DepthReached;
use crate::ui::hud::inventory::{EnchantChoice, ForgeButton, TradeOffer, TradeUpdated};
use crate::ui::hud::progress::JustEarned;
use crate::world::ClientWorldMap;

/// Shortest time between two armour sounds of a player: a suit swapped piece by piece in a few
/// clicks is heard, not a burst.
const ARMOUR_INTERVAL_SECS: f32 = 0.12;
/// Shortest time between two creaks of a bow drawn by a player.
const DRAW_INTERVAL_SECS: f32 = 0.4;
/// How near a player or a creature an arrow in flight was last seen for its end to be a hit,
/// in blocks beyond the half width of the creature: an arrow flies two blocks a tick.
const HIT_REACH: f32 = 2.0;
/// How near a player a stuck arrow was for its end to be its picking up.
const PICKUP_REACH: f32 = 3.0;
/// How long a mob must have been out of the water for its fall into it to splash: one that
/// swims at the surface bobs in and out.
const DRY_SECS: f32 = 1.0;
/// Shortest time between two splashes of a mob.
const SPLASH_INTERVAL_SECS: f32 = 3.0;

/// Where the ears are: the camera of the player, not one of the portals'.
fn ears(
    cameras: &Query<&GlobalTransform, (With<Camera3d>, Without<PortalCamera>)>,
) -> Option<Vec3> {
    cameras.iter().next().map(GlobalTransform::translation)
}

/// `sound` at `at`, if the ears are near enough to hear it.
fn heard(ears: Option<Vec3>, sound: Sound, at: Vec3) -> Option<PlaySound> {
    ears.is_some_and(|ears| ears.distance(at) <= HEARING_DISTANCE)
        .then(|| PlaySound::at(sound, at))
}

/// The sounds of the armour of a player going from `before` to `after`: a piece put on sounds
/// of its material, one taken off too, but one gone as the player was hurt broke.
pub fn armour_sounds(before: WornArmour, after: WornArmour, hurt: bool) -> Vec<Sound> {
    let material = |item: shared::world::ItemId| item.armour().map(|armour| armour.material);
    let mut sounds = Vec::new();
    for (was, is) in before.0.into_iter().zip(after.0) {
        let sound = match (was, is) {
            (Some(_), None) if hurt => Some(Sound::ArmourBreak),
            (Some(was), None) => material(was).map(Sound::ArmourOff),
            (was, Some(is)) if was != Some(is) => material(is).map(Sound::ArmourOn),
            _ => None,
        };
        if let Some(sound) = sound.filter(|sound| !sounds.contains(sound)) {
            sounds.push(sound);
        }
    }
    sounds
}

/// What a player was last seen wearing, its health, its bow drawn, and when its armour and its
/// bow were last heard.
#[derive(Default)]
pub struct PlayerSeen {
    worn: WornArmour,
    health: u32,
    bow_draw_ms: u32,
    parries: u8,
    armour_heard: f32,
    draw_heard: f32,
}

/// The armour put on, taken off and broken, and the bows drawn, of every player: the local
/// player's as it predicts them, the others' where they are.
#[allow(clippy::type_complexity)]
pub fn player_cues(
    time: Res<Time>,
    players: Query<(
        &Player,
        Option<&Worn>,
        Option<&HeldItem>,
        Has<CurrentPlayerMarker>,
    )>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<PortalCamera>)>,
    mut seen: Local<HashMap<PlayerId, PlayerSeen>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let now = time.elapsed_secs();
    let ears = ears(&cameras);
    seen.retain(|id, _| players.iter().any(|(player, ..)| player.id == *id));
    for (player, worn, held, local) in &players {
        let held = held.and_then(|held| held.item());
        let worn = match (local, worn) {
            (true, _) => player.inventory.worn(),
            (false, Some(worn)) => worn.0,
            (false, None) => WornArmour::default(),
        };
        let Some(last) = seen.get_mut(&player.id) else {
            seen.insert(
                player.id,
                PlayerSeen {
                    worn,
                    health: player.health,
                    bow_draw_ms: player.bow_draw_ms,
                    parries: player.parries,
                    ..default()
                },
            );
            continue;
        };
        let play = |sound| match local {
            true => Some(PlaySound::here(sound)),
            false => heard(ears, sound, player.position),
        };
        // A player who dies drops its armour: no sound of it
        if worn != last.worn && player.health > 0 && now - last.armour_heard >= ARMOUR_INTERVAL_SECS
        {
            let hurt = player.health < last.health;
            sounds.write_batch(
                armour_sounds(last.worn, worn, hurt)
                    .into_iter()
                    .filter_map(play),
            );
            last.armour_heard = now;
        }
        if last.bow_draw_ms == 0
            && player.bow_draw_ms > 0
            && now - last.draw_heard >= DRAW_INTERVAL_SECS
        {
            let draw = if held == Some(ItemId::Crossbow) {
                Sound::CrossbowLoad
            } else {
                Sound::BowDraw
            };
            sounds.write_batch(play(draw));
            last.draw_heard = now;
        }
        // A loaded crossbow let go
        if last.bow_draw_ms >= CROSSBOW_LOAD_MS
            && player.bow_draw_ms == 0
            && held == Some(ItemId::Crossbow)
        {
            sounds.write_batch(play(Sound::CrossbowShot));
        }
        if player.parries != last.parries {
            sounds.write_batch(play(Sound::ShieldBlock));
        }
        last.parries = player.parries;
        last.worn = worn;
        last.health = player.health;
        last.bow_draw_ms = player.bow_draw_ms;
    }
}

/// How an arrow seen last at `at`, in flight or stuck, ended, by what was near it then: in a
/// player or a creature it flew into, or in the hands of a player who picked it up.
pub fn arrow_end(
    at: Vec3,
    flying: bool,
    players: &[Vec3],
    creatures: &[(Vec3, f32)],
) -> Option<Sound> {
    if !flying {
        return players
            .iter()
            .any(|player| player.distance(at) <= PICKUP_REACH)
            .then_some(Sound::ArrowPickup);
    }
    let player = players
        .iter()
        .map(|player| player.distance(at))
        .fold(f32::MAX, f32::min);
    let creature = creatures
        .iter()
        .map(|(position, half_width)| position.distance(at) - half_width)
        .fold(f32::MAX, f32::min);
    match player.min(creature) {
        nearest if nearest > HIT_REACH => None,
        _ if player <= creature => Some(Sound::ArrowHitPlayer),
        _ => Some(Sound::ArrowHitCreature),
    }
}

/// The end of each arrow: a hit on a player or a creature, or its picking up. Its sticking in a
/// block is heard with its flight (`creatures::creature_calls`).
pub fn arrow_cues(
    mobs: Query<(Entity, &Mob, &Transform)>,
    players: Query<&Player>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<PortalCamera>)>,
    // Where each arrow was last seen, and whether it flew
    mut arrows: Local<HashMap<Entity, (Vec3, bool)>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let mut live = HashSet::new();
    let mut creatures = Vec::new();
    for (entity, mob, transform) in &mobs {
        if mob.kind == MobKind::Arrow {
            live.insert(entity);
            let flying = mob.action != shared::world::MobAction::Idle;
            arrows.insert(entity, (transform.translation, flying));
        } else if !mob.kind.is_projectile() {
            creatures.push((transform.translation, mob.kind.size().x * mob.scale / 2.0));
        }
    }
    let gone: Vec<_> = arrows
        .keys()
        .filter(|arrow| !live.contains(*arrow))
        .copied()
        .collect();
    if gone.is_empty() {
        return;
    }
    let ears = ears(&cameras);
    let players: Vec<Vec3> = players.iter().map(|player| player.position).collect();
    for arrow in gone {
        let (at, flying) = arrows.remove(&arrow).unwrap();
        if let Some(sound) = arrow_end(at, flying, &players, &creatures) {
            sounds.write_batch(heard(ears, sound, at));
        }
    }
}

/// What an animal was last seen as: whether it wanted a mate, was tamed, was in water, and for
/// how long it has been out of it.
#[derive(Component, Debug)]
pub struct MobCues {
    in_love: bool,
    tamed: bool,
    saddled: bool,
    /// Where it was the frame before, and when a boat's oars were last heard.
    last_at: Vec3,
    paddled_at: f32,
    in_water: bool,
    dry_secs: f32,
    splashed_at: f32,
}

/// A boat faster than this is rowed, in blocks/s, a stroke every so many seconds.
const PADDLE_SPEED: f32 = 2.0;
const PADDLE_INTERVAL_SECS: f32 = 0.8;

/// Whether the hull of a boat centred on `at` is in water.
fn boat_on_water(at: Vec3, world: &ClientWorldMap) -> bool {
    let hull = at - Vec3::Y * (MobKind::Boat.size().y / 2.0 - 0.05);
    world
        .blocks
        .get_block(hull.floor().as_ivec3())
        .is_some_and(|block| block.id == BlockId::Water)
}

/// Whether the feet of `mob`, whose hitbox is centred on `at`, are in water.
fn feet_in_water(mob: &Mob, at: Vec3, world: &ClientWorldMap) -> bool {
    let feet = at - Vec3::Y * (mob.kind.size().y * mob.scale / 2.0 - 0.2);
    world
        .blocks
        .get_block(feet.floor().as_ivec3())
        .is_some_and(|block| block.id == BlockId::Water)
}

/// The hearts rising over an animal, its birth and its taming, and the splash of a mob falling
/// into water.
pub fn mob_cues(
    mut commands: Commands,
    time: Res<Time>,
    world: Res<ClientWorldMap>,
    mut mobs: Query<(Entity, &Mob, &Transform, Option<&mut MobCues>)>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<PortalCamera>)>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let now = time.elapsed_secs();
    let ears = ears(&cameras);
    for (entity, mob, transform, cues) in &mut mobs {
        if mob.kind.is_projectile() {
            continue;
        }
        let at = transform.translation;
        let in_water = feet_in_water(mob, at, &world);
        let Some(mut cues) = cues else {
            commands.entity(entity).insert(MobCues {
                in_love: mob.in_love,
                tamed: mob.tamed,
                saddled: mob.saddled,
                last_at: at,
                paddled_at: f32::MIN,
                in_water,
                dry_secs: 0.0,
                splashed_at: f32::MIN,
            });
            if mob.born {
                sounds.write_batch(heard(ears, Sound::Birth, at));
            }
            continue;
        };
        if mob.in_love && !cues.in_love {
            sounds.write_batch(heard(ears, Sound::Love, at));
        }
        if mob.tamed && !cues.tamed {
            sounds.write_batch(heard(ears, Sound::Tamed, at));
        }
        if mob.saddled && !cues.saddled {
            sounds.write_batch(heard(ears, Sound::Saddle, at));
        }
        // A boat going on the water is rowed
        let delta = time.delta_secs();
        let speed = (at - cues.last_at).with_y(0.0).length() / delta.max(1e-3);
        if mob.kind == MobKind::Boat
            && speed > PADDLE_SPEED
            && boat_on_water(at, &world)
            && now - cues.paddled_at >= PADDLE_INTERVAL_SECS
        {
            sounds.write_batch(heard(ears, Sound::BoatPaddle, at));
            cues.paddled_at = now;
        }
        cues.last_at = at;
        cues.saddled = mob.saddled;
        if in_water
            && !cues.in_water
            && cues.dry_secs >= DRY_SECS
            && now - cues.splashed_at >= SPLASH_INTERVAL_SECS
        {
            sounds.write_batch(heard(ears, Sound::MobSplash, at));
            cues.splashed_at = now;
        }
        cues.dry_secs = if in_water {
            0.0
        } else {
            cues.dry_secs + time.delta_secs()
        };
        cues.in_love = mob.in_love;
        cues.tamed = mob.tamed;
        cues.in_water = in_water;
    }
}

/// A pop for every item the local player picks up, a light drop for an orb: once a frame each,
/// however many it picks up at once.
pub fn pickup_sounds(
    mut pickups: MessageReader<ItemPickedUp>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let mut heard = Vec::new();
    for pickup in pickups.read() {
        let sound = if pickup.orb {
            Sound::Orb
        } else {
            Sound::Pickup
        };
        if !heard.contains(&sound) {
            heard.push(sound);
        }
    }
    sounds.write_batch(heard.into_iter().map(PlaySound::here));
}

/// A chord as the local player goes up a level.
pub fn level_cues(
    players: Query<&Player, With<CurrentPlayerMarker>>,
    mut level: Local<Option<u32>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let Ok(player) = players.single() else {
        *level = None;
        return;
    };
    let now = player.experience.level;
    if level.is_some_and(|was| now > was) {
        sounds.write(PlaySound::here(Sound::LevelUp));
    }
    *level = Some(now);
}

/// A trade opening, and the villager's voice where it stands; an offer taken, as one of the
/// offers left goes down.
pub fn trade_cues(
    mut updates: MessageReader<TradeUpdated>,
    entities: Res<MobEntities>,
    mobs: Query<&Transform, With<Mob>>,
    // The trade open, and how many of each offer were left
    mut open: Local<Option<(MobId, Vec<u8>)>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    for TradeUpdated(update) in updates.read() {
        let Some(view) = update else {
            *open = None;
            continue;
        };
        if view.opening {
            sounds.write(PlaySound::here(Sound::TradeOpen));
            let villager = entities
                .get(view.mob)
                .and_then(|entity| mobs.get(entity).ok());
            if let Some(transform) = villager {
                sounds.write(PlaySound::at(
                    Sound::Creature(view.kind, CreatureCall::Cry),
                    transform.translation,
                ));
            }
        } else if let Some((_, left)) = open.as_ref().filter(|(mob, _)| *mob == view.mob) {
            if view.left.iter().zip(left).any(|(now, was)| now < was) {
                sounds.write(PlaySound::here(Sound::TradeDone));
            }
        }
        *open = Some((view.mob, view.left.clone()));
    }
}

/// An offer or an enchantment clicked that cannot be had: its button, disabled, does nothing
/// else.
pub fn refused_click(
    click: On<Pointer<Click>>,
    buttons: Query<
        (Has<TradeOffer>, Has<EnchantChoice>, Has<ForgeButton>),
        With<InteractionDisabled>,
    >,
    mut sounds: MessageWriter<PlaySound>,
) {
    let sound = match buttons.get(click.entity) {
        Ok((true, _, _)) => Sound::TradeRefused,
        Ok((_, true, _) | (_, _, true)) => Sound::EnchantRefused,
        _ => return,
    };
    sounds.write(PlaySound::here(sound));
}

/// A low sting as the banner of a depth reached shows: before `depths::announce_depth`, which
/// takes the tier.
pub fn depth_cue(reached: Res<DepthReached>, mut sounds: MessageWriter<PlaySound>) {
    if reached.is_changed() && reached.0.is_some() {
        sounds.write(PlaySound::here(Sound::DepthReached));
    }
}

/// A chime with the toasts of achievements earned: before `progress::show_earned`, which shows
/// and forgets them.
pub fn achievement_cue(earned: Res<JustEarned>, mut sounds: MessageWriter<PlaySound>) {
    if earned.any() {
        sounds.write(PlaySound::here(Sound::Achievement));
    }
}

/// A drum as the bar of a boss appears.
pub fn boss_cue(near: Res<NearBoss>, mut shown: Local<bool>, mut sounds: MessageWriter<PlaySound>) {
    let now = near.0.is_some();
    if now && !*shown {
        sounds.write(PlaySound::here(Sound::BossBar));
    }
    *shown = now;
}

/// The credits starting: in a game they follow the victory, whose fanfare opens them; from the
/// menu a softer chord.
pub fn credits_cue(
    added: Query<(), Added<Credits>>,
    state: Res<State<GameState>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if added.is_empty() {
        return;
    }
    sounds.write(PlaySound::here(match state.get() {
        GameState::Game => Sound::Victory,
        _ => Sound::Credits,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::ItemId;

    fn suit(items: [Option<ItemId>; 4]) -> WornArmour {
        WornArmour(items)
    }

    #[test]
    fn armour_put_on_and_taken_off_sounds_of_its_material_and_breaks_in_a_hurt() {
        use shared::players::armour::ArmourMaterial::*;
        let none = suit([None; 4]);
        let helmet = suit([Some(ItemId::LeatherHelmet), None, None, None]);
        let iron = suit([
            Some(ItemId::IronHelmet),
            Some(ItemId::IronChestplate),
            None,
            None,
        ]);
        assert_eq!(
            armour_sounds(none, helmet, false),
            [Sound::ArmourOn(Leather)]
        );
        assert_eq!(
            armour_sounds(helmet, none, false),
            [Sound::ArmourOff(Leather)]
        );
        assert_eq!(armour_sounds(helmet, none, true), [Sound::ArmourBreak]);
        // Two pieces at once are heard once
        assert_eq!(armour_sounds(none, iron, false), [Sound::ArmourOn(Iron)]);
        assert!(armour_sounds(iron, iron, true).is_empty());
    }

    #[test]
    fn an_arrow_ends_in_what_was_nearest_or_in_a_hand() {
        let player = [Vec3::new(0.0, 0.0, 0.0)];
        let sheep = [(Vec3::new(10.0, 0.0, 0.0), 0.45)];
        let near = |x| Vec3::new(x, 0.0, 0.0);
        assert_eq!(
            arrow_end(near(1.0), true, &player, &sheep),
            Some(Sound::ArrowHitPlayer)
        );
        assert_eq!(
            arrow_end(near(9.0), true, &player, &sheep),
            Some(Sound::ArrowHitCreature)
        );
        // Gone far from anything, as out of sight
        assert_eq!(arrow_end(near(5.0), true, &player, &sheep), None);
        assert_eq!(
            arrow_end(near(1.0), false, &player, &sheep),
            Some(Sound::ArrowPickup)
        );
        assert_eq!(arrow_end(near(9.0), false, &player, &sheep), None);
    }

    #[test]
    fn an_orb_picked_up_drops_lightly_and_many_at_once_are_heard_once() {
        let mut app = App::new();
        app.add_message::<PlaySound>()
            .add_message::<ItemPickedUp>()
            .add_systems(Update, pickup_sounds);
        for orb in [true, true, false] {
            app.world_mut().write_message(ItemPickedUp { orb });
        }
        app.update();
        let heard: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PlaySound>>()
            .drain()
            .collect();
        assert_eq!(
            heard,
            [PlaySound::here(Sound::Orb), PlaySound::here(Sound::Pickup)]
        );
    }
}
