//! The music: pieces made by `scripts/generate_music.py`, in `data/music/`, played one at a time
//! with minutes of silence between them, as a game of open worlds plays its music. Each is
//! chosen for the mood of the moment it starts in: the title in the menus, then the day, the
//! night, the underground, or the boss when the game asks for it (`MusicMoodOverride`). It
//! fades in, plays through, and fades out early only when the moment changes in kind: the
//! player leaves the menus, or a boss fight starts or ends.

use std::ops::Range;

use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;
use rand::{seq::SliceRandom, Rng};
use shared::world::hostiles::is_night;

use super::settings::{AudioSettings, Channel};
use crate::portal::PortalCamera;
use crate::state::GameState;
use crate::world::time::ClientTime;

/// What the music should feel like.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MusicMood {
    /// The menus.
    Title,
    Day,
    Night,
    /// The player is deep in the ground (`UNDERGROUND_BELOW`).
    Underground,
    /// A boss fight: tense.
    Boss,
}

impl MusicMood {
    /// The pieces of the mood, relative to the assets folder.
    pub fn pieces(self) -> &'static [&'static str] {
        match self {
            MusicMood::Title => &["music/title.ogg"],
            MusicMood::Day => &["music/day1.ogg", "music/day2.ogg"],
            MusicMood::Night => &["music/night1.ogg", "music/night2.ogg"],
            MusicMood::Underground => &["music/underground1.ogg", "music/underground2.ogg"],
            MusicMood::Boss => &["music/boss.ogg"],
        }
    }

    /// Moods of different kinds do not wait for each other: a piece of one is faded out when
    /// the other is wanted. The day, the night and the underground are one kind.
    fn kind(self) -> MoodKind {
        match self {
            MusicMood::Title => MoodKind::Menus,
            MusicMood::Boss => MoodKind::Boss,
            _ => MoodKind::World,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoodKind {
    Menus,
    World,
    Boss,
}

/// A mood the game imposes on the music, over the one of the time and place: a boss sets it to
/// `Some(MusicMood::Boss)` as its fight starts, and back to `None` when it ends.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicMoodOverride(pub Option<MusicMood>);

/// Below this height the ears are underground, in blocks: deeper than the valleys and the lakes,
/// the sea being at 62.
pub const UNDERGROUND_BELOW: f32 = 50.0;

/// The mood of the moment: the title in the menus; in the game the one imposed, or the
/// underground below `UNDERGROUND_BELOW`, else the night or the day at `tick`.
pub fn mood_of(
    in_game: bool,
    imposed: MusicMoodOverride,
    tick: u64,
    ears: Option<Vec3>,
) -> MusicMood {
    if !in_game {
        return MusicMood::Title;
    }
    if let Some(mood) = imposed.0 {
        return mood;
    }
    if ears.is_some_and(|ears| ears.y < UNDERGROUND_BELOW) {
        MusicMood::Underground
    } else if is_night(tick) {
        MusicMood::Night
    } else {
        MusicMood::Day
    }
}

/// Seconds a piece rises over.
pub const FADE_IN: f32 = 6.0;
/// Seconds a piece cut short falls over.
pub const FADE_OUT: f32 = 4.0;
/// A piece still playing after this many seconds is taken as over: it may never have loaded.
/// The longest lasts under 3 minutes.
const LONGEST_PIECE: f32 = 240.0;

/// Seconds of silence after a piece in the world: a few minutes.
const WORLD_SILENCE: Range<f32> = 180.0..360.0;
/// Seconds of silence after the title piece, which the menus play again and again.
const MENUS_SILENCE: Range<f32> = 60.0..120.0;
/// Seconds of silence after a piece of the boss, which keeps the fight going.
const BOSS_SILENCE: Range<f32> = 3.0..6.0;
/// Seconds of silence before the first piece of a mood of another kind: soon in the menus and
/// in a boss fight, after a while in the world.
fn silence_before(kind: MoodKind, rng: &mut impl Rng) -> f32 {
    match kind {
        MoodKind::Menus => 2.0,
        MoodKind::Boss => 1.0,
        MoodKind::World => rng.gen_range(20.0..60.0),
    }
}

fn silence_after(kind: MoodKind, rng: &mut impl Rng) -> f32 {
    rng.gen_range(match kind {
        MoodKind::Menus => MENUS_SILENCE,
        MoodKind::World => WORLD_SILENCE,
        MoodKind::Boss => BOSS_SILENCE,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Nothing plays, for `left` seconds more, before a piece of a mood of the kind `kind`.
    Silent { left: f32, kind: MoodKind },
    /// A piece of `mood` plays since `elapsed` seconds; it falls for `fading` seconds more
    /// when cut short.
    Playing {
        mood: MusicMood,
        elapsed: f32,
        fading: Option<f32>,
    },
}

/// What the music should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicCommand {
    /// Start the piece of this file.
    Start(&'static str),
    /// Stop the piece playing.
    Stop,
}

/// When the music plays and what: one piece at a time, the silences between them, and the fades.
#[derive(Clone, Debug, PartialEq)]
pub struct MusicScheduler {
    phase: Phase,
    last_piece: Option<&'static str>,
}

impl Default for MusicScheduler {
    /// The silence before the title piece, as the game opens.
    fn default() -> Self {
        Self {
            phase: Phase::Silent {
                left: 2.0,
                kind: MoodKind::Menus,
            },
            last_piece: None,
        }
    }
}

impl MusicScheduler {
    /// Lets `seconds` pass while `wanted` is the mood of the moment; `finished` says whether the
    /// piece playing came to its end. Returns what the music should do now.
    pub fn update(
        &mut self,
        seconds: f32,
        wanted: MusicMood,
        finished: bool,
        rng: &mut impl Rng,
    ) -> Option<MusicCommand> {
        match &mut self.phase {
            Phase::Silent { left, kind } => {
                if *kind != wanted.kind() {
                    // Another kind of moment: its own wait, not what was left of the last one
                    *kind = wanted.kind();
                    *left = silence_before(*kind, rng);
                }
                *left -= seconds;
                if *left > 0.0 {
                    return None;
                }
                let piece = self.pick(wanted, rng);
                self.phase = Phase::Playing {
                    mood: wanted,
                    elapsed: 0.0,
                    fading: None,
                };
                Some(MusicCommand::Start(piece))
            }
            Phase::Playing {
                mood,
                elapsed,
                fading,
            } => {
                *elapsed += seconds;
                let kind = mood.kind();
                if finished || *elapsed > LONGEST_PIECE {
                    self.phase = Phase::Silent {
                        left: silence_after(kind, rng),
                        kind,
                    };
                    return Some(MusicCommand::Stop);
                }
                if let Some(left) = fading {
                    *left -= seconds;
                    if *left <= 0.0 {
                        let wanted = wanted.kind();
                        self.phase = Phase::Silent {
                            left: silence_before(wanted, rng),
                            kind: wanted,
                        };
                        return Some(MusicCommand::Stop);
                    }
                } else if kind != wanted.kind() {
                    *fading = Some(FADE_OUT);
                }
                None
            }
        }
    }

    /// A piece of `mood`, not the one played last when the mood has others.
    fn pick(&mut self, mood: MusicMood, rng: &mut impl Rng) -> &'static str {
        let pieces = mood.pieces();
        let others: Vec<&'static str> = pieces
            .iter()
            .copied()
            .filter(|piece| Some(*piece) != self.last_piece)
            .collect();
        let choices = if others.is_empty() { pieces } else { &others };
        let piece = *choices.choose(rng).expect("every mood has a piece");
        self.last_piece = Some(piece);
        piece
    }

    /// The share of its volume the piece playing is heard at: rising as it starts, falling as
    /// it is cut short; 0 in the silences.
    pub fn gain(&self) -> f32 {
        match self.phase {
            Phase::Silent { .. } => 0.0,
            Phase::Playing {
                elapsed, fading, ..
            } => (elapsed / FADE_IN).min(1.0) * fading.map_or(1.0, |left| left / FADE_OUT),
        }
    }

    /// Whether a piece plays.
    pub fn is_playing(&self) -> bool {
        matches!(self.phase, Phase::Playing { .. })
    }
}

/// The music's scheduler, and the entity of the piece playing.
#[derive(Resource, Default)]
pub struct Music {
    pub scheduler: MusicScheduler,
    piece: Option<Entity>,
}

/// The entity playing a piece of music.
#[derive(Component)]
pub struct MusicPiece;

pub fn music_plugin(app: &mut App) {
    app.init_resource::<Music>()
        .init_resource::<MusicMoodOverride>()
        .add_systems(Update, play_music);
}

#[allow(clippy::too_many_arguments)]
fn play_music(
    mut commands: Commands,
    time: Res<Time>,
    mut music: ResMut<Music>,
    imposed: Res<MusicMoodOverride>,
    state: Res<State<GameState>>,
    clock: Option<Res<ClientTime>>,
    settings: Res<AudioSettings>,
    asset_server: Res<AssetServer>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<PortalCamera>)>,
    mut pieces: Query<Option<&mut AudioSink>, With<MusicPiece>>,
) {
    let in_game = *state.get() == GameState::Game;
    let ears = cameras.iter().next().map(GlobalTransform::translation);
    let tick = clock.map_or(0, |clock| clock.0);
    let wanted = mood_of(in_game, *imposed, tick, ears);
    // A piece played through despawns
    let finished = music.piece.is_some_and(|piece| !pieces.contains(piece));
    let mut rng = rand::thread_rng();
    let command = music
        .scheduler
        .update(time.delta_secs(), wanted, finished, &mut rng);
    let volume = music.scheduler.gain() * settings.volume_of(Channel::Music);
    match command {
        Some(MusicCommand::Start(file)) => {
            if let Some(old) = music.piece.take() {
                commands.entity(old).try_despawn();
            }
            let entity = commands
                .spawn((
                    AudioPlayer::new(asset_server.load(file)),
                    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
                    MusicPiece,
                ))
                .id();
            music.piece = Some(entity);
        }
        Some(MusicCommand::Stop) => {
            if let Some(old) = music.piece.take() {
                commands.entity(old).try_despawn();
            }
        }
        None => {
            if let Some(Ok(Some(mut sink))) = music.piece.map(|piece| pieces.get_mut(piece)) {
                sink.set_volume(Volume::Linear(volume));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use shared::{DAY_DURATION_IN_TICKS, MIDNIGHT_IN_DAY, NOON_IN_DAY};

    const STEP: f32 = 0.5;

    #[test]
    fn the_menus_play_the_title_and_the_world_its_time_and_place() {
        let none = MusicMoodOverride::default();
        let up = Some(Vec3::new(0.0, 80.0, 0.0));
        let deep = Some(Vec3::new(0.0, 20.0, 0.0));
        assert_eq!(mood_of(false, none, NOON_IN_DAY, up), MusicMood::Title);
        assert_eq!(mood_of(true, none, NOON_IN_DAY, up), MusicMood::Day);
        assert_eq!(mood_of(true, none, MIDNIGHT_IN_DAY, up), MusicMood::Night);
        assert_eq!(mood_of(true, none, NOON_IN_DAY, deep), MusicMood::Underground);
        assert_eq!(
            mood_of(true, none, MIDNIGHT_IN_DAY + DAY_DURATION_IN_TICKS, deep),
            MusicMood::Underground
        );
    }

    #[test]
    fn a_boss_imposes_its_mood_anywhere_in_the_world_but_not_in_the_menus() {
        let boss = MusicMoodOverride(Some(MusicMood::Boss));
        let deep = Some(Vec3::new(0.0, 20.0, 0.0));
        assert_eq!(mood_of(true, boss, NOON_IN_DAY, deep), MusicMood::Boss);
        assert_eq!(mood_of(true, boss, MIDNIGHT_IN_DAY, None), MusicMood::Boss);
        assert_eq!(mood_of(false, boss, NOON_IN_DAY, deep), MusicMood::Title);
    }

    /// What the scheduler does over `seconds` while `wanted` is wanted, each piece lasting
    /// `piece_seconds`: the times it starts and stops pieces, and whether it ever started one
    /// while another played.
    fn run(
        scheduler: &mut MusicScheduler,
        rng: &mut StdRng,
        wanted: MusicMood,
        seconds: f32,
        piece_seconds: f32,
        log: &mut Vec<(f32, MusicCommand)>,
        now: &mut f32,
    ) {
        let end = *now + seconds;
        let mut playing_since = log
            .iter()
            .rev()
            .find(|(_, command)| matches!(command, MusicCommand::Start(_)))
            .map(|(at, _)| *at);
        while *now < end {
            *now += STEP;
            let finished =
                scheduler.is_playing() && playing_since.is_some_and(|at| *now - at > piece_seconds);
            if let Some(command) = scheduler.update(STEP, wanted, finished, rng) {
                if let MusicCommand::Start(_) = command {
                    let playing = log.last().is_some_and(|(_, last)| {
                        matches!(last, MusicCommand::Start(_))
                    });
                    assert!(!playing, "two pieces at once at {now}: {log:?}");
                    playing_since = Some(*now);
                }
                log.push((*now, command));
            }
        }
    }

    #[test]
    fn pieces_in_the_world_come_one_at_a_time_minutes_apart() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut scheduler = MusicScheduler::default();
        let (mut log, mut now) = (Vec::new(), 0.0);
        run(&mut scheduler, &mut rng, MusicMood::Day, 3600.0, 160.0, &mut log, &mut now);
        let starts: Vec<f32> = log
            .iter()
            .filter(|(_, c)| matches!(c, MusicCommand::Start(_)))
            .map(|(at, _)| *at)
            .collect();
        // The first within a minute, then one every 160 s of piece and 3 to 6 minutes of silence
        assert!(starts[0] <= 60.5, "{starts:?}");
        assert!((6..=11).contains(&starts.len()), "{starts:?}");
        for pair in starts.windows(2) {
            let gap = pair[1] - pair[0];
            assert!(gap >= 160.0 + WORLD_SILENCE.start, "{gap}");
            assert!(gap <= 161.0 + STEP + WORLD_SILENCE.end, "{gap}");
        }
        // Starts and stops alternate: never two pieces at once
        for pair in log.windows(2) {
            assert_ne!(
                matches!(pair[0].1, MusicCommand::Start(_)),
                matches!(pair[1].1, MusicCommand::Start(_)),
                "{log:?}"
            );
        }
    }

    #[test]
    fn the_day_plays_its_own_pieces_and_never_the_same_twice_in_a_row() {
        let mut rng = StdRng::seed_from_u64(2);
        let mut scheduler = MusicScheduler::default();
        let (mut log, mut now) = (Vec::new(), 0.0);
        run(&mut scheduler, &mut rng, MusicMood::Day, 5000.0, 150.0, &mut log, &mut now);
        let played: Vec<&str> = log
            .iter()
            .filter_map(|(_, c)| match c {
                MusicCommand::Start(piece) => Some(*piece),
                MusicCommand::Stop => None,
            })
            .collect();
        assert!(played.iter().all(|piece| MusicMood::Day.pieces().contains(piece)));
        for pair in played.windows(2) {
            assert_ne!(pair[0], pair[1]);
        }
    }

    #[test]
    fn a_piece_fades_in_and_the_silences_are_silent() {
        let mut rng = StdRng::seed_from_u64(3);
        let mut scheduler = MusicScheduler::default();
        assert_eq!(scheduler.gain(), 0.0);
        let mut started = None;
        for _ in 0..10 {
            if let Some(command) = scheduler.update(STEP, MusicMood::Title, false, &mut rng) {
                started = Some(command);
                break;
            }
        }
        assert_eq!(started, Some(MusicCommand::Start("music/title.ogg")));
        assert_eq!(scheduler.gain(), 0.0);
        scheduler.update(FADE_IN / 2.0, MusicMood::Title, false, &mut rng);
        assert!((scheduler.gain() - 0.5).abs() < 1e-6);
        scheduler.update(FADE_IN, MusicMood::Title, false, &mut rng);
        assert_eq!(scheduler.gain(), 1.0);
    }

    #[test]
    fn leaving_the_menus_fades_the_title_out_and_the_world_waits_a_while() {
        let mut rng = StdRng::seed_from_u64(4);
        let mut scheduler = MusicScheduler::default();
        let (mut log, mut now) = (Vec::new(), 0.0);
        run(&mut scheduler, &mut rng, MusicMood::Title, 30.0, 150.0, &mut log, &mut now);
        assert!(scheduler.is_playing());
        // In the world, the title falls over FADE_OUT seconds, then stops
        assert_eq!(scheduler.update(1.0, MusicMood::Day, false, &mut rng), None);
        assert_eq!(scheduler.gain(), 1.0);
        scheduler.update(1.0, MusicMood::Day, false, &mut rng);
        assert!((scheduler.gain() - 0.75).abs() < 1e-6, "{}", scheduler.gain());
        let stop = (0..20)
            .map(|_| scheduler.update(STEP, MusicMood::Day, false, &mut rng))
            .position(|command| command == Some(MusicCommand::Stop))
            .unwrap();
        assert_eq!(stop, 5, "4 s of fade, 1 already gone");
        assert_eq!(scheduler.gain(), 0.0);
        // Not a day piece at once: at least 20 s of silence
        for _ in 0..39 {
            assert_eq!(scheduler.update(STEP, MusicMood::Day, false, &mut rng), None);
        }
    }

    #[test]
    fn the_day_turning_to_night_lets_the_piece_play_through() {
        let mut rng = StdRng::seed_from_u64(5);
        let mut scheduler = MusicScheduler::default();
        let (mut log, mut now) = (Vec::new(), 0.0);
        run(&mut scheduler, &mut rng, MusicMood::Day, 70.0, 150.0, &mut log, &mut now);
        assert!(scheduler.is_playing());
        for _ in 0..100 {
            assert_eq!(scheduler.update(STEP, MusicMood::Night, false, &mut rng), None);
            assert_eq!(scheduler.gain(), 1.0);
        }
    }

    #[test]
    fn a_boss_cuts_the_piece_short_and_its_music_comes_at_once_and_again() {
        let mut rng = StdRng::seed_from_u64(6);
        let mut scheduler = MusicScheduler::default();
        let (mut log, mut now) = (Vec::new(), 0.0);
        run(&mut scheduler, &mut rng, MusicMood::Day, 70.0, 150.0, &mut log, &mut now);
        assert!(scheduler.is_playing());
        let before = log.len();
        run(&mut scheduler, &mut rng, MusicMood::Boss, 400.0, 150.0, &mut log, &mut now);
        let commands: Vec<MusicCommand> = log[before..].iter().map(|(_, c)| *c).collect();
        assert_eq!(commands[0], MusicCommand::Stop);
        assert_eq!(commands[1], MusicCommand::Start("music/boss.ogg"));
        // Faded out in 4 s, then a second of silence
        let (stopped, started) = (log[before].0, log[before + 1].0);
        assert!(stopped - 70.0 <= FADE_OUT + STEP, "{stopped}");
        assert!(started - stopped <= 1.0 + STEP, "{started} {stopped}");
        // Played through, it comes back within seconds
        assert_eq!(commands[3], MusicCommand::Start("music/boss.ogg"));
        assert!(log[before + 3].0 - log[before + 2].0 <= BOSS_SILENCE.end + STEP);
    }

    #[test]
    fn a_piece_that_never_ends_is_taken_as_over() {
        let mut rng = StdRng::seed_from_u64(7);
        let mut scheduler = MusicScheduler::default();
        let (mut log, mut now) = (Vec::new(), 0.0);
        // The entity of the piece never despawns
        run(&mut scheduler, &mut rng, MusicMood::Title, 300.0, f32::MAX, &mut log, &mut now);
        assert!(log.contains(&(log[1].0, MusicCommand::Stop)), "{log:?}");
        assert!(log[1].0 - log[0].0 <= LONGEST_PIECE + STEP);
    }

    #[test]
    fn every_piece_of_every_mood_is_in_the_data_folder() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        for mood in [
            MusicMood::Title,
            MusicMood::Day,
            MusicMood::Night,
            MusicMood::Underground,
            MusicMood::Boss,
        ] {
            for piece in mood.pieces() {
                assert!(data.join(piece).is_file(), "{mood:?}: missing {piece}");
            }
        }
    }
}
