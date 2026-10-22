//! The sounds of the weather: the thunder of each strike, heard after its flash as late as
//! sound travels, and the rain and the wind, three loops played all along whose volumes follow
//! the weather, what falls around, and where the player stands. Out in the open the rain hisses;
//! under a roof it drums, the higher the roof the softer; in a cave nothing is heard; under
//! water every sound of the weather is muffled.
//!
//! The thunder is no louder nor softer for its side, as the spatial sounds would make it: its
//! emitter is kept two blocks from the ears, toward the strike, where Bevy does not attenuate,
//! and only its balance between the ears follows the strike.

use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;
use rand::{seq::SliceRandom, Rng};
use shared::world::{BlockTransparency, ChunkMap, LIGHTNING_REACH, SNOW_LINE};

use super::settings::{AudioSettings, Channel};
use super::sounds::{Sound, SoundLibrary};
use crate::camera::eye_under_water;
use crate::state::GameState;
use crate::world::lightning::Strike;
use crate::world::precipitation::occlusion::{height_at, OcclusionMap, UNKNOWN};
use crate::world::rendering::climate::{precipitation_at, PrecipitationKind};
use crate::world::weather::ClientWeather;
use crate::world::{ClientWorldMap, MaterialResource};

/// The speed of sound, in blocks a second.
const SPEED_OF_SOUND: f32 = 340.;

/// Within this distance a strike cracks and booms; farther off it rumbles. The thunder is
/// heard at full volume within it, and fades with the distance beyond, down to `FAINTEST`, and
/// is not heard at all beyond `LIGHTNING_REACH`, where its flash fades out.
const CRACK_REACH: f32 = 48.;
const FAINTEST: f32 = 0.15;

/// How far from the ears the emitter of the thunder is kept, in blocks: within the radius where
/// Bevy does not attenuate (`audio::SPATIAL_SCALE`).
const EMITTER_DISTANCE: f32 = 2.;

/// Bevy's spatial sound plays a sound right ahead at three quarters of its volume in each ear,
/// up to the whole in the ear it comes from: this brings it ahead back to its volume.
const PAN_COMPENSATION: f32 = 4. / 3.;

/// How much the thunder is muffled from inside, closed in, and under water.
const THUNDER_INDOORS: f32 = 0.55;
const THUNDER_UNDER_WATER: f32 = 0.3;

/// How far the ears look for the open sky around them, in blocks, along `RAYS` directions.
const HEARING: f32 = 6.;
const RAYS: usize = 16;
const RAY_STEP: f32 = 0.25;

/// A roof this many blocks above the eye or less drums at full volume, one `ROOF_SILENT` above
/// is not heard (the roof of a cave).
const ROOF_LOUD: f32 = 4.;
const ROOF_SILENT: f32 = 24.;

/// How far apart the columns around the player are on which what falls is looked at.
const CLIMATE_SPREAD: i32 = 8;

/// The loudest the loops play, before the volume of the Weather channel.
const RAIN_VOLUME: f32 = 0.8;
const RAIN_INDOORS_VOLUME: f32 = 0.9;
const WIND_VOLUME: f32 = 0.7;

/// How much wind blows in falling snow, and in a thunderstorm.
const SNOW_WIND: f32 = 0.6;
const STORM_WIND: f32 = 0.5;

/// Under water, the drumming of the rain on the surface, as loud as this share of the rain.
const RAIN_UNDER_WATER: f32 = 0.5;

/// Seconds for the volume of a loop to go most of the way to a new one.
const EASE_SECS: f32 = 0.35;

/// Seconds after which a strike is heard `distance` blocks away.
pub fn thunder_delay(distance: f32) -> f32 {
    distance / SPEED_OF_SOUND
}

/// What a strike `distance` blocks away is heard as, and how loud, from 0 to 1; `None` beyond
/// `LIGHTNING_REACH`.
pub fn thunder_of(distance: f32) -> Option<(Sound, f32)> {
    if distance > LIGHTNING_REACH {
        return None;
    }
    let sound = if distance <= CRACK_REACH {
        Sound::Thunder
    } else {
        Sound::DistantThunder
    };
    let volume = (CRACK_REACH / distance.max(1.))
        .powf(0.75)
        .clamp(FAINTEST, 1.);
    Some((sound, volume))
}

/// Where the emitter of the thunder of a strike at `struck` is kept, heard from `ears`.
pub fn emitter_position(ears: Vec3, struck: Vec3) -> Vec3 {
    let toward = (struck - ears).with_y(0.).normalize_or(Vec3::NEG_Z);
    ears + toward * EMITTER_DISTANCE
}

/// How the place where the player stands lets the weather be heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shelter {
    /// The share of the directions around in which the open sky is within hearing, from 0,
    /// closed in, to 1, out in the open.
    pub open: f32,
    /// How far above the eye the roof is, in blocks; `None` out in the open.
    pub roof: Option<f32>,
}

impl Shelter {
    pub const OPEN: Self = Self {
        open: 1.,
        roof: None,
    };
}

/// How the place of the eye at `eye` lets the weather be heard, where `stop` gives the height at
/// which the rain stops on a column (`occlusion::height_at`) and `wall` whether a block stops the
/// sound. A column is open when the rain falls on it below the eye; the directions are looked
/// along at the height of the eye, through what is not a wall.
pub fn shelter(eye: Vec3, stop: impl Fn(i32, i32) -> f32, wall: impl Fn(IVec3) -> bool) -> Shelter {
    let column = eye.floor().as_ivec3();
    let open_column = |x: i32, z: i32| stop(x, z) <= eye.y;
    let own = stop(column.x, column.z);
    if own <= eye.y {
        return Shelter::OPEN;
    }
    let roof = (own < UNKNOWN).then_some(own - eye.y);
    let open = (0..RAYS)
        .filter(|&ray| {
            let angle = ray as f32 / RAYS as f32 * std::f32::consts::TAU;
            let direction = Vec3::new(angle.cos(), 0., angle.sin());
            let mut last = column;
            let mut travelled = RAY_STEP;
            while travelled <= HEARING {
                let block = (eye + direction * travelled).floor().as_ivec3();
                travelled += RAY_STEP;
                if block == last {
                    continue;
                }
                last = block;
                if wall(block) {
                    return false;
                }
                if open_column(block.x, block.z) {
                    return true;
                }
            }
            false
        })
        .count();
    Shelter {
        open: open as f32 / RAYS as f32,
        roof,
    }
}

/// What the weather around the player is, for its sounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surroundings {
    /// How much falls, from 0 to 1 (`ClientWeather::precipitation`).
    pub falling: f32,
    /// How stormy it is, from 0 to 1.
    pub thunder: f32,
    /// The shares of the columns around on which rain falls, and snow.
    pub rain: f32,
    pub snow: f32,
    pub shelter: Shelter,
    pub under_water: bool,
}

/// The volumes of the loops of the rain heard in the open, the rain heard under a roof, and the
/// wind, from 0 to 1, before the volume of the Weather channel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoopVolumes {
    pub rain: f32,
    pub rain_indoors: f32,
    pub wind: f32,
}

impl LoopVolumes {
    pub fn of(around: &Surroundings) -> Self {
        let rain = around.falling * around.rain;
        let wind = (around.falling * around.snow * SNOW_WIND).max(around.thunder * STORM_WIND);
        if around.under_water {
            return Self {
                rain: 0.,
                rain_indoors: rain * RAIN_UNDER_WATER * RAIN_INDOORS_VOLUME,
                wind: 0.,
            };
        }
        let Shelter { open, roof } = around.shelter;
        let drumming = roof.map_or(0., |height| {
            1. - crate::world::celestial::smoothstep(ROOF_LOUD, ROOF_SILENT, height)
        });
        Self {
            rain: rain * open * RAIN_VOLUME,
            rain_indoors: rain * drumming * RAIN_INDOORS_VOLUME,
            wind: wind * (0.35 + 0.65 * open) * WIND_VOLUME,
        }
    }

    fn get(&self, sound: Sound) -> f32 {
        match sound {
            Sound::Rain => self.rain,
            Sound::RainIndoors => self.rain_indoors,
            _ => self.wind,
        }
    }
}

/// `current` eased toward `target` over `secs` seconds.
pub fn ease(current: f32, target: f32, secs: f32) -> f32 {
    let eased = current + (target - current) * (1. - (-secs / EASE_SECS).exp());
    if (eased - target).abs() < 1e-3 {
        target
    } else {
        eased
    }
}

/// The gain the thunder of `volume` is played at: raised by `PAN_COMPENSATION`, so that ahead
/// it is heard at `volume`, but at most 1, as the ear it comes from plays it at its whole gain
/// and the thunder is recorded close to full scale.
fn thunder_gain(volume: f32) -> f32 {
    (volume * PAN_COMPENSATION).min(1.)
}

/// The volume of the thunder of a strike heard at `volume` from where the player is.
fn thunder_volume(volume: f32, shelter: Shelter, under_water: bool) -> f32 {
    if under_water {
        return volume * THUNDER_UNDER_WATER;
    }
    volume * (THUNDER_INDOORS + (1. - THUNDER_INDOORS) * shelter.open)
}

/// A loop of the weather, and its volume now, before the Weather channel.
#[derive(Component)]
pub(crate) struct WeatherLoop {
    pub sound: Sound,
    pub volume: f32,
}

/// The thunder of a strike on its way: it is heard once `in_secs` is over.
struct Rolling {
    struck: Vec3,
    distance: f32,
    in_secs: f32,
}

/// The thunders on their way.
#[derive(Resource, Default)]
struct Thunders(Vec<Rolling>);

/// The emitter of a thunder, kept toward its strike.
#[derive(Component)]
pub(crate) struct ThunderFrom(pub Vec3);

/// The ears of the player: where the camera is.
fn ears(cameras: &Query<&GlobalTransform, With<Camera3d>>) -> Option<Vec3> {
    cameras.iter().next().map(GlobalTransform::translation)
}

pub fn weather_sound_plugin(app: &mut App) {
    app.init_resource::<Thunders>()
        .add_systems(OnEnter(GameState::Game), spawn_weather_loops)
        .add_systems(OnExit(GameState::Game), |mut thunders: ResMut<Thunders>| {
            thunders.0.clear()
        })
        .add_systems(
            Update,
            (
                hear_strikes,
                roll_thunder,
                keep_thunder_toward_strike,
                play_weather_loops,
            )
                .chain()
                .run_if(in_state(GameState::Game)),
        );
}

fn spawn_weather_loops(mut commands: Commands, library: Res<SoundLibrary>) {
    for sound in [Sound::Rain, Sound::RainIndoors, Sound::Wind] {
        let Some(source) = library.0.get(&sound).and_then(|variants| variants.first()) else {
            continue;
        };
        commands.spawn((
            AudioPlayer::new(source.clone()),
            PlaybackSettings::LOOP
                .with_volume(Volume::Linear(0.))
                .paused(),
            WeatherLoop { sound, volume: 0. },
            DespawnOnExit(GameState::Game),
        ));
    }
}

fn hear_strikes(
    mut strikes: MessageReader<Strike>,
    mut thunders: ResMut<Thunders>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
) {
    let Some(ears) = ears(&cameras) else {
        strikes.clear();
        return;
    };
    for strike in strikes.read() {
        let struck = strike.at.as_vec3() + Vec3::new(0.5, 1., 0.5);
        let distance = struck.distance(ears);
        thunders.0.push(Rolling {
            struck,
            distance,
            in_secs: thunder_delay(distance),
        });
    }
}

/// What the player hears the weather through: where it stands, and whether under water.
fn place_of_ears(ears: Vec3, world_map: &ChunkMap, occlusion: Option<&Image>) -> (Shelter, bool) {
    let under_water = eye_under_water(world_map, ears);
    let shelter = match occlusion {
        Some(occlusion) => shelter(
            ears,
            |x, z| height_at(occlusion, x, z),
            |at| {
                world_map.get_block(at).is_some_and(|block| {
                    matches!(
                        block.id.get_visibility(),
                        BlockTransparency::Solid | BlockTransparency::Transparent
                    )
                })
            },
        ),
        None => Shelter::OPEN,
    };
    (shelter, under_water)
}

#[allow(clippy::too_many_arguments)]
fn roll_thunder(
    mut commands: Commands,
    time: Res<Time>,
    mut thunders: ResMut<Thunders>,
    library: Res<SoundLibrary>,
    settings: Res<AudioSettings>,
    world_map: Res<ClientWorldMap>,
    occlusion: Res<OcclusionMap>,
    images: Res<Assets<Image>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
) {
    if thunders.0.is_empty() {
        return;
    }
    let secs = time.delta_secs();
    let mut heard = Vec::new();
    thunders.0.retain_mut(|rolling| {
        rolling.in_secs -= secs;
        if rolling.in_secs > 0. {
            return true;
        }
        heard.push((rolling.struck, rolling.distance));
        false
    });
    let (Some(ears), false) = (ears(&cameras), heard.is_empty()) else {
        return;
    };
    let channel = settings.volume_of(Channel::Weather);
    let (shelter, under_water) =
        place_of_ears(ears, &world_map.blocks, images.get(&occlusion.image));
    let mut rng = rand::thread_rng();
    for (struck, distance) in heard {
        let Some((sound, volume)) = thunder_of(distance) else {
            continue;
        };
        let volume = thunder_volume(volume, shelter, under_water) * channel;
        let source = library
            .0
            .get(&sound)
            .and_then(|variants| variants.choose(&mut rng));
        let (Some(source), true) = (source, volume > 0.) else {
            continue;
        };
        commands.spawn((
            AudioPlayer::new(source.clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(thunder_gain(volume)))
                .with_speed(rng.gen_range(0.94..1.06))
                .with_spatial(true),
            Transform::from_translation(emitter_position(ears, struck)),
            ThunderFrom(struck),
            DespawnOnExit(GameState::Game),
        ));
    }
}

fn keep_thunder_toward_strike(
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut emitters: Query<(&ThunderFrom, &mut Transform)>,
) {
    let Some(ears) = ears(&cameras) else {
        return;
    };
    for (from, mut transform) in &mut emitters {
        transform.translation = emitter_position(ears, from.0);
    }
}

/// The shares of the columns around `ears` on which rain falls, and snow, after the climate
/// map: what falls high enough is snow.
fn falls_around(ears: Vec3, climate: Option<&Image>) -> (f32, f32) {
    let Some(climate) = climate else {
        return (1., 0.);
    };
    let column = ears.floor().as_ivec3();
    let (mut rain, mut snow, mut known) = (0, 0, 0);
    for dx in -1..=1 {
        for dz in -1..=1 {
            let (x, z) = (
                column.x + dx * CLIMATE_SPREAD,
                column.z + dz * CLIMATE_SPREAD,
            );
            match precipitation_at(climate, x, z) {
                Some(PrecipitationKind::Rain) if ears.y > SNOW_LINE as f32 => snow += 1,
                Some(PrecipitationKind::Rain) => rain += 1,
                Some(PrecipitationKind::Snow) => snow += 1,
                Some(PrecipitationKind::None) => {}
                None => continue,
            }
            known += 1;
        }
    }
    if known == 0 {
        return (0., 0.);
    }
    (rain as f32 / known as f32, snow as f32 / known as f32)
}

#[allow(clippy::too_many_arguments)]
fn play_weather_loops(
    time: Res<Time>,
    weather: Res<ClientWeather>,
    settings: Res<AudioSettings>,
    world_map: Res<ClientWorldMap>,
    occlusion: Res<OcclusionMap>,
    materials: Option<Res<MaterialResource>>,
    images: Res<Assets<Image>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut loops: Query<(&mut WeatherLoop, Option<&mut AudioSink>)>,
) {
    let heard = weather.precipitation() > 0. || weather.thunder > 0.;
    let targets = match ears(&cameras) {
        Some(ears) if heard => {
            let (shelter, under_water) =
                place_of_ears(ears, &world_map.blocks, images.get(&occlusion.image));
            let climate = materials
                .as_ref()
                .and_then(|materials| images.get(&materials.climate_map));
            let (rain, snow) = falls_around(ears, climate);
            LoopVolumes::of(&Surroundings {
                falling: weather.precipitation(),
                thunder: weather.thunder,
                rain,
                snow,
                shelter,
                under_water,
            })
        }
        _ => LoopVolumes::default(),
    };
    let channel = settings.volume_of(Channel::Weather);
    for (mut playing, sink) in &mut loops {
        playing.volume = ease(
            playing.volume,
            targets.get(playing.sound),
            time.delta_secs(),
        );
        let Some(mut sink) = sink else {
            continue;
        };
        let volume = playing.volume * channel;
        if volume <= 0. {
            if !sink.is_paused() {
                sink.pause();
            }
            continue;
        }
        sink.set_volume(Volume::Linear(volume));
        if sink.is_paused() {
            sink.play();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::precipitation::occlusion::OPEN;
    use std::collections::HashSet;

    #[test]
    fn the_thunder_comes_after_the_flash_as_late_as_sound_travels() {
        assert_eq!(thunder_delay(0.), 0.);
        assert_eq!(thunder_delay(340.), 1.);
        assert!((thunder_delay(160.) - 0.47).abs() < 0.01);
    }

    #[test]
    fn a_strike_close_by_cracks_and_one_far_off_rumbles_softer_with_the_distance() {
        assert_eq!(thunder_of(20.), Some((Sound::Thunder, 1.)));
        assert_eq!(thunder_of(48.), Some((Sound::Thunder, 1.)));
        let mut before = 1.;
        for distance in [60., 100., 160., 400.] {
            let (sound, volume) = thunder_of(distance).unwrap();
            assert_eq!(sound, Sound::DistantThunder);
            assert!(volume < before && volume >= FAINTEST, "{distance}");
            before = volume;
        }
        assert_eq!(thunder_of(1_400.).unwrap().1, FAINTEST);
        assert_eq!(thunder_of(2_000.), None);
    }

    #[test]
    fn the_thunder_is_heard_from_the_side_of_its_strike_two_blocks_away() {
        let ears = Vec3::new(10., 70., 10.);
        let emitter = emitter_position(ears, Vec3::new(110., 60., 10.));
        assert_eq!(emitter, Vec3::new(12., 70., 10.));
        // a strike right on the player
        assert_eq!(emitter_position(ears, ears).distance(ears), 2.);
    }

    #[test]
    fn the_thunder_is_raised_for_the_balance_of_the_ears_but_never_past_full_volume() {
        assert!((thunder_gain(0.3) - 0.4).abs() < 1e-6);
        assert!((thunder_gain(0.75) - 1.).abs() < 1e-6);
        // from the side, an ear plays it at the whole of its gain: never louder than the file
        assert_eq!(thunder_gain(1.), 1.);
        assert_eq!(thunder_gain(0.), 0.);
    }

    #[test]
    fn the_thunder_is_muffled_inside_and_under_water() {
        assert_eq!(thunder_volume(0.8, Shelter::OPEN, false), 0.8);
        let closed = Shelter {
            open: 0.,
            roof: Some(3.),
        };
        assert!((thunder_volume(0.8, closed, false) - 0.44).abs() < 1e-5);
        assert!(thunder_volume(0.8, Shelter::OPEN, true) < thunder_volume(0.8, closed, false));
    }

    /// A flat ground at y 64 with the blocks `blocks` on it, which stop the rain and the sound.
    fn place(blocks: &[IVec3], eye: Vec3) -> Shelter {
        let blocks: HashSet<IVec3> = blocks.iter().copied().collect();
        let stop = |x: i32, z: i32| {
            blocks
                .iter()
                .filter(|b| b.x == x && b.z == z)
                .map(|b| b.y as f32 + 1.)
                .fold(64., f32::max)
        };
        shelter(eye, stop, |at| blocks.contains(&at))
    }

    /// The blocks of a box of planks from `min` to `max`, walls, floor and roof, hollow.
    fn hut(min: IVec3, max: IVec3, door: Option<IVec3>) -> Vec<IVec3> {
        let mut blocks = Vec::new();
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    let at = IVec3::new(x, y, z);
                    let shell = x == min.x || x == max.x || y == max.y || z == min.z || z == max.z;
                    if shell && Some(at) != door && Some(at - IVec3::Y) != door {
                        blocks.push(at);
                    }
                }
            }
        }
        blocks
    }

    const EYE: Vec3 = Vec3::new(0.5, 65.6, 0.5);

    #[test]
    fn out_in_the_open_and_by_a_wall_the_rain_is_all_around() {
        assert_eq!(place(&[], EYE), Shelter::OPEN);
        let wall: Vec<IVec3> = (-5..=5)
            .flat_map(|z| (64..68).map(move |y| IVec3::new(1, y, z)))
            .collect();
        assert_eq!(place(&wall, EYE), Shelter::OPEN);
    }

    #[test]
    fn in_a_closed_room_the_open_sky_is_nowhere_and_through_a_door_a_little() {
        let closed = place(
            &hut(IVec3::new(-3, 65, -3), IVec3::new(3, 68, 3), None),
            EYE,
        );
        assert_eq!(closed.open, 0.);
        assert_eq!(closed.roof, Some(69. - EYE.y));
        let door = IVec3::new(3, 65, 0);
        let open = place(
            &hut(IVec3::new(-3, 65, -3), IVec3::new(3, 68, 3), Some(door)),
            EYE,
        );
        assert!(open.open > 0. && open.open < 0.25, "{open:?}");
    }

    #[test]
    fn under_a_pavilion_the_rain_is_heard_all_around_and_on_the_roof() {
        let roof: Vec<IVec3> = (-2..=2)
            .flat_map(|x| (-2..=2).map(move |z| IVec3::new(x, 68, z)))
            .collect();
        let pavilion = place(&roof, EYE);
        assert_eq!(pavilion.open, 1.);
        assert_eq!(pavilion.roof, Some(69. - EYE.y));
    }

    #[test]
    fn in_a_cave_the_rain_is_far_above() {
        // the eye at 40, under a rock 40 blocks thick
        let eye = Vec3::new(0.5, 40.6, 0.5);
        let mut rock = Vec::new();
        for x in -8..=8 {
            for z in -8..=8 {
                for y in [39, 80] {
                    rock.push(IVec3::new(x, y, z));
                }
                if x.abs() == 8 || z.abs() == 8 {
                    rock.extend((40..80).map(|y| IVec3::new(x, y, z)));
                }
            }
        }
        let cave = place(&rock, eye);
        assert_eq!(cave.open, 0.);
        assert!(cave.roof.unwrap() > ROOF_SILENT);
    }

    fn rain(shelter: Shelter) -> Surroundings {
        Surroundings {
            falling: 1.,
            thunder: 0.,
            rain: 1.,
            snow: 0.,
            shelter,
            under_water: false,
        }
    }

    #[test]
    fn without_rain_nor_storm_the_loops_are_silent() {
        let clear = Surroundings {
            falling: 0.,
            ..rain(Shelter::OPEN)
        };
        assert_eq!(LoopVolumes::of(&clear), LoopVolumes::default());
    }

    #[test]
    fn the_rain_hisses_in_the_open_and_drums_on_a_low_roof() {
        let open = LoopVolumes::of(&rain(Shelter::OPEN));
        assert_eq!(
            (open.rain, open.rain_indoors, open.wind),
            (RAIN_VOLUME, 0., 0.)
        );
        let room = LoopVolumes::of(&rain(Shelter {
            open: 0.,
            roof: Some(3.),
        }));
        assert_eq!((room.rain, room.rain_indoors), (0., RAIN_INDOORS_VOLUME));
        let hall = LoopVolumes::of(&rain(Shelter {
            open: 0.,
            roof: Some(12.),
        }));
        assert!(hall.rain_indoors > 0. && hall.rain_indoors < room.rain_indoors);
        let cave = LoopVolumes::of(&rain(Shelter {
            open: 0.,
            roof: Some(40.),
        }));
        assert_eq!(cave, LoopVolumes::default());
        let pavilion = LoopVolumes::of(&rain(Shelter {
            open: 1.,
            roof: Some(3.),
        }));
        assert_eq!(
            (pavilion.rain, pavilion.rain_indoors),
            (RAIN_VOLUME, RAIN_INDOORS_VOLUME)
        );
    }

    #[test]
    fn the_snow_and_the_storm_bring_the_wind_and_the_water_muffles_everything() {
        let snow = LoopVolumes::of(&Surroundings {
            rain: 0.,
            snow: 1.,
            ..rain(Shelter::OPEN)
        });
        assert_eq!((snow.rain, snow.rain_indoors), (0., 0.));
        assert!((snow.wind - SNOW_WIND * WIND_VOLUME).abs() < 1e-6);
        let storm = LoopVolumes::of(&Surroundings {
            thunder: 1.,
            ..rain(Shelter::OPEN)
        });
        assert!((storm.wind - STORM_WIND * WIND_VOLUME).abs() < 1e-6);
        let sheltered = LoopVolumes::of(&Surroundings {
            thunder: 1.,
            ..rain(Shelter {
                open: 0.,
                roof: Some(3.),
            })
        });
        assert!(sheltered.wind < storm.wind && sheltered.wind > 0.);
        let under = LoopVolumes::of(&Surroundings {
            thunder: 1.,
            under_water: true,
            ..rain(Shelter::OPEN)
        });
        assert_eq!((under.rain, under.wind), (0., 0.));
        assert!(under.rain_indoors > 0. && under.rain_indoors < RAIN_INDOORS_VOLUME);
    }

    #[test]
    fn the_volumes_ease_toward_their_targets() {
        let mut volume = 0.;
        for _ in 0..10 {
            let next = ease(volume, 1., 0.05);
            assert!(next > volume && next < 1.);
            volume = next;
        }
        for _ in 0..200 {
            volume = ease(volume, 1., 0.05);
        }
        assert_eq!(volume, 1.);
        assert_eq!(ease(0.5, 0.5, 0.016), 0.5);
    }

    #[test]
    fn leaving_a_world_forgets_its_lightning_its_rain_and_its_thunder() {
        use crate::world::lightning::render::LightningDraw;
        use crate::world::lightning::{lightning_plugin, Lightning};
        use crate::world::precipitation::precipitation_plugin;
        use crate::world::precipitation::render::{PrecipitationDraw, PrecipitationView};
        use crate::world::{BlockChanged, ChunkLoadEvent, GraphicsSettings, RenderDistance};
        use bevy::state::app::StatesPlugin;
        use shared::world::WeatherKind;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<Shader>()
            .init_asset::<AudioSource>()
            .init_state::<GameState>()
            .add_message::<ChunkLoadEvent>()
            .add_message::<BlockChanged>()
            .insert_resource(ClientWeather::settled(WeatherKind::Thunder))
            .init_resource::<GraphicsSettings>()
            .init_resource::<RenderDistance>()
            .init_resource::<MaterialResource>()
            .init_resource::<ClientWorldMap>()
            .init_resource::<AudioSettings>()
            .insert_resource(SoundLibrary(
                [Sound::Rain, Sound::RainIndoors, Sound::Wind]
                    .into_iter()
                    .map(|sound| (sound, vec![Handle::default()]))
                    .collect(),
            ))
            .add_plugins((lightning_plugin, precipitation_plugin, weather_sound_plugin));
        app.world_mut().spawn((
            Camera3d::default(),
            PrecipitationView,
            GlobalTransform::from_translation(Vec3::new(0.5, 80., 0.5)),
        ));
        let go_to = |app: &mut App, state| {
            app.world_mut()
                .resource_mut::<NextState<GameState>>()
                .set(state);
            app.update();
        };
        go_to(&mut app, GameState::Game);
        app.world_mut().write_message(Strike {
            at: IVec3::new(20, 64, 0),
            seed: 1,
        });
        app.update();
        let loops = |app: &mut App| {
            let world = app.world_mut();
            world.query::<&WeatherLoop>().iter(world).count()
        };
        assert_eq!(app.world().resource::<Lightning>().strikes.len(), 1);
        assert!(app.world().resource::<LightningDraw>().0.is_some());
        assert!(app.world().resource::<PrecipitationDraw>().0.is_some());
        assert_eq!(app.world().resource::<Thunders>().0.len(), 1);
        assert_eq!(loops(&mut app), 3);

        go_to(&mut app, GameState::Menu);
        assert!(app.world().resource::<Lightning>().strikes.is_empty());
        assert!(app.world().resource::<LightningDraw>().0.is_none());
        assert!(app.world().resource::<PrecipitationDraw>().0.is_none());
        assert!(app.world().resource::<Thunders>().0.is_empty());
        assert_eq!(loops(&mut app), 0);
    }

    #[test]
    fn a_column_unknown_is_no_open_sky_nor_a_roof() {
        let unknown = shelter(EYE, |_, _| UNKNOWN, |_| false);
        assert_eq!(
            unknown,
            Shelter {
                open: 0.,
                roof: None
            }
        );
        assert_eq!(shelter(EYE, |_, _| OPEN, |_| false), Shelter::OPEN);
    }
}
