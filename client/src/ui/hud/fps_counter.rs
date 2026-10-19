//! The FPS counter in the top-right corner of the game, shown as the player chose in the Interface
//! settings or with its key: the frame rate alone, or over a graph of the frame times of the last
//! seconds, which shows the stutters an average hides.

use std::collections::VecDeque;

use bevy::prelude::*;
use shared::GameFolderPaths;

use super::debug::{debug_font, debug_line, fps_color, FpsText};
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::state::{GameState, KeyMap};
use crate::ui::settings::{write_interface_settings, FpsCounterChoice, InterfaceSettings};
use crate::ui::theme::{BEVEL_HI, MUTED, PANEL, SLOT, SPACE_S, SPACE_XS, TEXT};

/// The time a bar of the graph covers, in ms. A bar shows the worst frame of its time, so that
/// no stutter is averaged away, and many frames share a bar at a high frame rate.
const BUCKET_MS: u128 = 50;
/// The bars of the graph, the oldest on the left: 10 s.
const BARS: usize = 200;
const BAR_WIDTH: f32 = 2.0;
const GRAPH_HEIGHT: f32 = 120.0;
/// The frame time at the top of the graph; longer frames are cut there.
const SCALE_MS: f32 = 40.0;
/// The frame rates marked by a line across the graph.
const GUIDE_FPS: [f32; 2] = [30.0, 60.0];

/// The root of the FPS counter.
#[derive(Component)]
pub struct FpsCounter;

/// The graph under the frame rate, with its stats.
#[derive(Component)]
pub struct FpsGraph;

/// The bar `n` of the graph, from the left.
#[derive(Component)]
pub struct FrameBar(usize);

/// A stat under the graph, over the frames of the graph.
#[derive(Component, Clone, Copy)]
pub enum GraphStat {
    Average,
    /// The average frame rate of the slowest 1 % of the frames.
    OnePercentLow,
    WorstFrame,
}

/// A frame of the game: the bucket of `BUCKET_MS` it ended in, and how long it took.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameTime {
    bucket: u64,
    ms: f32,
}

/// The frame times of the last `BARS` buckets, the oldest first.
#[derive(Resource, Default)]
pub struct FrameTimes(VecDeque<FrameTime>);

impl FrameTimes {
    /// Adds a frame of `ms` that ended in `bucket`, and drops those too old for the graph.
    fn record(&mut self, bucket: u64, ms: f32) {
        self.0.push_back(FrameTime { bucket, ms });
        while self
            .0
            .front()
            .is_some_and(|frame| frame.bucket + (BARS as u64) < bucket)
        {
            self.0.pop_front();
        }
    }
}

/// What the graph shows of the buckets before `current`.
#[derive(Debug, PartialEq)]
struct FrameGraph {
    /// The worst frame time of each bucket, the oldest first, if a frame ended in it.
    bars: Vec<Option<f32>>,
    stats: Option<FrameStats>,
}

#[derive(Debug, PartialEq)]
struct FrameStats {
    average_fps: f32,
    one_percent_low_fps: f32,
    worst_ms: f32,
}

/// The graph of the last `BARS` buckets that ended before `current`, the one still running.
fn frame_graph(frames: &VecDeque<FrameTime>, current: u64) -> FrameGraph {
    let mut bars = vec![None; BARS];
    let mut shown = Vec::new();
    for frame in frames {
        if frame.bucket >= current || current - frame.bucket > BARS as u64 {
            continue;
        }
        let bar = &mut bars[BARS - (current - frame.bucket) as usize];
        *bar = Some(bar.map_or(frame.ms, |worst: f32| worst.max(frame.ms)));
        shown.push(frame.ms);
    }
    if shown.is_empty() {
        return FrameGraph { bars, stats: None };
    }
    shown.sort_by(|a, b| b.total_cmp(a));
    let total: f32 = shown.iter().sum();
    let slowest = &shown[..(shown.len() / 100).max(1)];
    let slowest_total: f32 = slowest.iter().sum();
    let stats = FrameStats {
        average_fps: 1000.0 * shown.len() as f32 / total,
        one_percent_low_fps: 1000.0 * slowest.len() as f32 / slowest_total,
        worst_ms: shown[0],
    };
    FrameGraph {
        bars,
        stats: Some(stats),
    }
}

/// The height of the bar of a frame of `ms`.
fn bar_height(ms: f32) -> f32 {
    (ms.min(SCALE_MS) / SCALE_MS * GRAPH_HEIGHT).max(1.0)
}

/// The bucket the time `elapsed` is in.
fn bucket_of(elapsed: std::time::Duration) -> u64 {
    (elapsed.as_millis() / BUCKET_MS) as u64
}

/// Spawns the FPS counter, its number updated by `fps_text_update_system` like the one of the
/// debug overlay.
pub fn spawn_fps_counter(mut commands: Commands, assets: Res<AssetServer>) {
    let font = debug_font(&assets);
    let muted = |text: String| (Text::new(text), font.clone(), TextColor(MUTED));
    let root = commands
        .spawn((
            FpsCounter,
            DespawnOnExit(GameState::Game),
            BackgroundColor(PANEL),
            // Shown by `show_fps_counter`, and under the panels like the debug overlay
            Visibility::Hidden,
            GlobalZIndex(-1),
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Percent(1.),
                top: Val::Percent(1.),
                padding: UiRect::all(px(SPACE_XS)),
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_S),
                ..default()
            },
            children![(
                debug_line("FPS: ", TEXT, font.clone()),
                children![(
                    FpsText,
                    TextSpan::new(" N/A"),
                    font.clone(),
                    TextColor(TEXT)
                )],
            )],
        ))
        .id();
    let graph = commands
        .spawn((
            FpsGraph,
            Node {
                display: Display::None,
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_XS),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let plot = commands
        .spawn((
            Node {
                width: px(BAR_WIDTH * BARS as f32),
                height: px(GRAPH_HEIGHT),
                align_items: AlignItems::FlexEnd,
                ..default()
            },
            BackgroundColor(SLOT),
            ChildOf(graph),
        ))
        .id();
    for fps in GUIDE_FPS {
        let height = bar_height(1000.0 / fps);
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(height),
                height: px(1),
                ..default()
            },
            BackgroundColor(BEVEL_HI),
            ChildOf(plot),
        ));
        commands.spawn((
            muted(format!("{fps:.0} FPS · {:.0} ms", 1000.0 / fps)),
            Node {
                position_type: PositionType::Absolute,
                left: px(SPACE_XS),
                bottom: px(height + 1.0),
                ..default()
            },
            ChildOf(plot),
        ));
    }
    for n in 0..BARS {
        commands.spawn((
            FrameBar(n),
            Node {
                width: px(BAR_WIDTH),
                height: px(0),
                ..default()
            },
            BackgroundColor(TEXT),
            ChildOf(plot),
        ));
    }
    let seconds = BARS as u128 * BUCKET_MS / 1000;
    commands.spawn((
        Node {
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
        ChildOf(graph),
        children![muted(format!("-{seconds} s")), muted("now".into())],
    ));
    let stats = commands
        .spawn((
            Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
                column_gap: px(SPACE_S),
                ..default()
            },
            ChildOf(graph),
        ))
        .id();
    for (stat, label) in [
        (GraphStat::Average, "AVERAGE"),
        (GraphStat::OnePercentLow, "1% LOW"),
        (GraphStat::WorstFrame, "WORST FRAME"),
    ] {
        commands.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(stats),
            children![
                muted(label.into()),
                (stat, Text::new("N/A"), font.clone(), TextColor(TEXT)),
            ],
        ));
    }
}

/// Shows the FPS counter, and its graph, as the settings ask.
pub fn show_fps_counter(
    settings: Res<InterfaceSettings>,
    mut counters: Query<&mut Visibility, With<FpsCounter>>,
    mut graphs: Query<&mut Node, With<FpsGraph>>,
) {
    let shown = if settings.fps_counter == FpsCounterChoice::Off {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };
    for mut visibility in &mut counters {
        visibility.set_if_neq(shown);
    }
    let display = if settings.fps_counter == FpsCounterChoice::Graph {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut graphs {
        if node.display != display {
            node.display = display;
        }
    }
}

/// Shows the next choice of the FPS counter on its key, and saves it.
pub fn cycle_fps_counter_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    mut settings: ResMut<InterfaceSettings>,
    paths: Res<GameFolderPaths>,
) {
    if !is_action_just_pressed(GameAction::CycleFpsCounter, &keyboard_input, &key_map) {
        return;
    }
    settings.fps_counter = settings.fps_counter.next();
    write_interface_settings(&settings, &paths);
}

/// Starts the frame times of a world from nothing.
pub fn reset_frame_times(mut commands: Commands) {
    commands.insert_resource(FrameTimes::default());
}

/// Records the time of the last frame, in real time, which goes on in the pause menu.
pub fn record_frame_time(time: Res<Time<Real>>, mut frames: ResMut<FrameTimes>) {
    frames.record(bucket_of(time.elapsed()), time.delta_secs() * 1000.0);
}

/// Draws the graph again each time a bucket ends, while it is shown.
pub fn draw_fps_graph(
    settings: Res<InterfaceSettings>,
    time: Res<Time<Real>>,
    frames: Res<FrameTimes>,
    mut drawn: Local<Option<u64>>,
    mut bars: Query<(&FrameBar, &mut Node, &mut BackgroundColor)>,
    mut stats: Query<(&GraphStat, &mut Text, &mut TextColor)>,
) {
    let current = bucket_of(time.elapsed());
    if settings.fps_counter != FpsCounterChoice::Graph || *drawn == Some(current) {
        return;
    }
    *drawn = Some(current);
    let graph = frame_graph(&frames.0, current);
    for (FrameBar(n), mut node, mut color) in &mut bars {
        let (height, bar_color) = match graph.bars[*n] {
            Some(ms) => (bar_height(ms), fps_color(1000.0 / ms as f64)),
            None => (0.0, TEXT),
        };
        node.height = px(height);
        color.0 = bar_color;
    }
    for (stat, mut text, mut color) in &mut stats {
        let Some(values) = &graph.stats else {
            text.0 = "N/A".into();
            color.0 = TEXT;
            continue;
        };
        let (shown, fps) = match stat {
            GraphStat::Average => (format!("{:.0} FPS", values.average_fps), values.average_fps),
            GraphStat::OnePercentLow => (
                format!("{:.0} FPS", values.one_percent_low_fps),
                values.one_percent_low_fps,
            ),
            GraphStat::WorstFrame => (
                format!("{:.1} ms", values.worst_ms),
                1000.0 / values.worst_ms,
            ),
        };
        text.0 = shown;
        color.0 = fps_color(fps as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(list: &[(u64, f32)]) -> FrameTimes {
        let mut times = FrameTimes::default();
        for &(bucket, ms) in list {
            times.record(bucket, ms);
        }
        times
    }

    #[test]
    fn a_bar_shows_the_worst_frame_of_its_bucket_and_now_is_on_the_right() {
        let times = frames(&[(7, 7.0), (8, 7.0), (8, 30.0), (9, 6.0), (10, 50.0)]);
        let graph = frame_graph(&times.0, 10);
        assert_eq!(graph.bars[BARS - 1], Some(6.0));
        assert_eq!(graph.bars[BARS - 2], Some(30.0));
        assert_eq!(graph.bars[BARS - 3], Some(7.0));
        // the bucket still running waits for its end
        assert!(graph.bars[..BARS - 3].iter().all(Option::is_none));
        let stats = graph.stats.unwrap();
        assert_eq!(stats.worst_ms, 30.0);
        assert_eq!(stats.average_fps, 4000.0 / 50.0);
    }

    #[test]
    fn the_one_percent_low_is_the_rate_of_the_slowest_frames() {
        let mut list = vec![(1, 5.0); 99];
        list.push((1, 50.0));
        let stats = frame_graph(&frames(&list).0, 2).stats.unwrap();
        assert_eq!(stats.one_percent_low_fps, 20.0);
        assert_eq!(stats.average_fps, 1000.0 * 100.0 / (99.0 * 5.0 + 50.0));
    }

    #[test]
    fn the_frames_older_than_the_graph_are_dropped() {
        let last = BARS as u64 + 5;
        let times = frames(&[(4, 9.0), (5, 8.0), (last, 7.0)]);
        assert_eq!(
            times.0.iter().map(|frame| frame.bucket).collect::<Vec<_>>(),
            vec![5, last]
        );
        let graph = frame_graph(&times.0, last + 1);
        assert_eq!(graph.bars[0], None);
        assert_eq!(graph.bars[BARS - 1], Some(7.0));
    }

    #[test]
    fn no_frame_gives_no_stats() {
        assert_eq!(frame_graph(&VecDeque::new(), 3).stats, None);
    }

    #[test]
    fn the_bars_are_cut_at_the_top_of_the_graph() {
        assert_eq!(bar_height(SCALE_MS * 2.0), GRAPH_HEIGHT);
        assert_eq!(bar_height(SCALE_MS / 2.0), GRAPH_HEIGHT / 2.0);
        assert_eq!(bar_height(0.0), 1.0);
    }
}
