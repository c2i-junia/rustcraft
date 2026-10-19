//! The status of the listed servers: asked on threads when the screen opens, after an Add or an
//! Edit of the address and on Refresh (F5), and shown on their cards and in the detail pane as the
//! answers come.

use std::collections::HashMap;

use bevy::{prelude::*, ui::Selected};

use super::super::{text, MenuState};
use super::{allow_join, show_detail, ServerCard, ServerFacts};
use crate::network::address::parse_server_address;
use crate::network::status::{latency_bars, InFlight, ServerStatus, StatusProbe};
use crate::ui::assets::FONT_PATH;
use crate::ui::theme::{BEVEL_HI, DANGER, FONT_S, MUTED, OK, SPACE_S, TEXT};
use crate::ui::widgets::dialog::dialog_open;
use crate::ui::widgets::text_field::text_field_focused;

/// The side of the status dot of a card.
const DOT_SIDE: f32 = 10.0;
/// The width of a latency bar, and the height the shortest adds to each next one.
const BAR_STEP: f32 = 4.0;
/// The room between two latency bars.
const BAR_GAP: f32 = 2.0;
/// How many latency bars a card shows, filled or not.
const BAR_COUNT: u8 = 4;

/// The status of each server by `ServerAddress::key` (the `ip` itself when it does not parse),
/// on the screen's root: kept across the rebuilds of the cards, dropped with the screen.
#[derive(Component, Default)]
pub struct StatusBoard(HashMap<String, Probe>);

/// What the board knows of a server.
struct Probe {
    status: ServerStatus,
    /// The probe asking it, until its answer is taken. Replaced, it is dropped: its answer is
    /// never taken.
    in_flight: Option<InFlight<ServerStatus>>,
    /// Whether `status` changed since `collect` last returned the key.
    changed: bool,
}

/// The key of the server at `ip` (a `ServerItem::ip`) on the board.
fn status_key(ip: &str) -> String {
    parse_server_address(ip).map_or_else(|_| ip.to_string(), |address| address.key())
}

impl StatusBoard {
    /// Starts asking `ip` with `probe`, replacing (and so abandoning) a probe in flight.
    pub fn ping(&mut self, ip: &str, probe: StatusProbe) {
        let typed = ip.to_string();
        let in_flight = InFlight::spawn(move || (probe.0)(&typed));
        self.0.insert(
            status_key(ip),
            Probe {
                status: ServerStatus::Pinging,
                in_flight: Some(in_flight),
                changed: true,
            },
        );
    }

    /// Forgets every server but those at `ips`, dropping their probes in flight.
    pub fn keep_only<'a>(&mut self, ips: impl Iterator<Item = &'a str>) {
        let kept: Vec<String> = ips.map(status_key).collect();
        self.0.retain(|key, _| kept.contains(key));
    }

    /// Whether `ip` was ever asked.
    pub fn knows(&self, ip: &str) -> bool {
        self.0.contains_key(&status_key(ip))
    }

    /// What is known of `ip`: `Pinging` until it answers, or when it was never asked.
    pub fn status(&self, ip: &str) -> ServerStatus {
        self.0
            .get(&status_key(ip))
            .map_or(ServerStatus::Pinging, |probe| probe.status.clone())
    }

    /// The keys whose probe finished since the last call, and those asked again since, which
    /// are pinging once more.
    pub fn collect(&mut self) -> Vec<String> {
        self.0
            .iter_mut()
            .filter_map(|(key, probe)| {
                if let Some(status) = probe.in_flight.as_ref().and_then(InFlight::take) {
                    probe.status = status;
                    probe.in_flight = None;
                    probe.changed = true;
                }
                std::mem::take(&mut probe.changed).then(|| key.clone())
            })
            .collect()
    }
}

/// The right side of a card: the players and the latency bars, or the status in a word. It is
/// built again when the status changes.
#[derive(Component)]
pub struct CardStatus;

/// The dot on the left of a card, in the colour of its status.
#[derive(Component)]
pub struct StatusDot;

/// The latency bars of a card, of which the first `.0` are filled.
#[derive(Component)]
pub struct LatencyBars(pub u8);

/// The word and colour of a status on a card, and the two lines of the pane (Decision 7).
pub fn card_status(status: &ServerStatus) -> (String, Color) {
    match status {
        ServerStatus::Online {
            players,
            max_players,
            ..
        } => (format!("{players} / {max_players}"), TEXT),
        ServerStatus::Pinging => ("Pinging...".to_string(), MUTED),
        ServerStatus::Offline => ("Offline".to_string(), MUTED),
        ServerStatus::Incompatible => ("Incompatible".to_string(), DANGER),
        ServerStatus::UnknownHost => ("Unknown host".to_string(), DANGER),
        ServerStatus::BadAddress => ("Bad address".to_string(), DANGER),
    }
}

/// The headline of a status in the pane, its colour, and the line under it (empty: none).
pub fn pane_status(status: &ServerStatus) -> (String, Color, String) {
    let (headline, color, line) = match status {
        ServerStatus::Online {
            players,
            max_players,
            latency,
        } => {
            let headline = format!("Online \u{b7} {players} / {max_players} players");
            let ms = format!("{} ms", latency.as_millis());
            return (headline, OK, ms);
        }
        ServerStatus::Pinging => ("Pinging...", MUTED, ""),
        ServerStatus::Offline => ("Offline", MUTED, "No answer. Check the address or refresh."),
        ServerStatus::Incompatible => (
            "Incompatible version",
            DANGER,
            "This server runs another version of Rustcraft.",
        ),
        ServerStatus::UnknownHost => ("Unknown host", DANGER, "Check the address."),
        ServerStatus::BadAddress => ("Bad address", DANGER, "Edit the server to fix it."),
    };
    (headline.to_string(), color, line.to_string())
}

/// The colour of the dot of a card: green online, grey while it is unknown or offline, red when
/// the server cannot be joined as it is.
fn dot_color(status: &ServerStatus) -> Color {
    match status {
        ServerStatus::Online { .. } => OK,
        ServerStatus::Pinging | ServerStatus::Offline => MUTED,
        ServerStatus::Incompatible | ServerStatus::UnknownHost | ServerStatus::BadAddress => DANGER,
    }
}

/// Whether a server of this status may be joined: one of another version, or with an address
/// that is not one, would only time out.
pub(super) fn can_join(status: &ServerStatus) -> bool {
    !matches!(
        status,
        ServerStatus::Incompatible | ServerStatus::BadAddress
    )
}

/// The status dot of a card.
pub(super) fn status_dot(status: &ServerStatus) -> impl Bundle {
    (
        StatusDot,
        Node {
            width: px(DOT_SIDE),
            height: px(DOT_SIDE),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(dot_color(status)),
    )
}

/// Shows `status` in `side`, the right side of a card, in place of what it showed.
pub(super) fn show_card_status(
    commands: &mut Commands,
    side: Entity,
    status: &ServerStatus,
    font: &Handle<Font>,
) {
    commands.entity(side).despawn_related::<Children>();
    let (word, color) = card_status(status);
    commands.spawn((text(&word, FONT_S, color, font), ChildOf(side)));
    if let ServerStatus::Online { latency, .. } = status {
        commands.spawn((
            latency_bars_of(LatencyBars(latency_bars(*latency))),
            ChildOf(side),
        ));
    }
}

/// The latency bars `bars`: rising from left to right, the first `bars.0` filled.
fn latency_bars_of(bars: LatencyBars) -> impl Bundle {
    let filled = bars.0;
    (
        bars,
        Node {
            align_items: AlignItems::FlexEnd,
            column_gap: px(BAR_GAP),
            ..default()
        },
        Children::spawn(SpawnIter((0..BAR_COUNT).map(move |bar| {
            let fill = if bar < filled { OK } else { BEVEL_HI };
            (
                Node {
                    width: px(BAR_STEP),
                    height: px(BAR_STEP * f32::from(bar + 1)),
                    ..default()
                },
                BackgroundColor(fill),
            )
        }))),
    )
}

/// The right side of a card, empty until `show_card_status` fills it.
pub(super) fn card_status_node() -> impl Bundle {
    (
        CardStatus,
        Node {
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(SPACE_S),
            ..default()
        },
    )
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        // the cards say they are pinging in the frame of F5, before any answer can come
        (
            refresh_on_f5
                .run_if(not(dialog_open))
                .run_if(not(text_field_focused)),
            collect_statuses,
        )
            .chain()
            .run_if(in_state(MenuState::Multi)),
    );
}

/// Asks every listed server again, abandoning the probes in flight.
pub(super) fn ping_every_server(
    board: &mut StatusBoard,
    cards: &Query<&ServerCard>,
    probe: StatusProbe,
) {
    for card in cards {
        board.ping(&card.0.ip, probe);
    }
}

/// Takes the answers of the probes, and shows the statuses that changed on their cards, and in
/// the pane for the selected one.
fn collect_statuses(
    mut boards: Query<&mut StatusBoard>,
    cards: Query<(Entity, &ServerCard, Has<Selected>)>,
    children: Query<&Children>,
    dots: Query<(), With<StatusDot>>,
    card_statuses: Query<(), With<CardStatus>>,
    facts: Query<Entity, With<ServerFacts>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(mut board) = boards.single_mut() else {
        return;
    };
    let changed = board.collect();
    if changed.is_empty() {
        return;
    }
    let font: Handle<Font> = assets.load(FONT_PATH);
    for (card, server, selected) in &cards {
        if !changed.contains(&status_key(&server.0.ip)) {
            continue;
        }
        let status = board.status(&server.0.ip);
        for &part in children.get(card).into_iter().flatten() {
            if dots.contains(part) {
                commands
                    .entity(part)
                    .insert(BackgroundColor(dot_color(&status)));
            } else if card_statuses.contains(part) {
                show_card_status(&mut commands, part, &status, &font);
            }
        }
        if selected {
            if let Ok(facts) = facts.single() {
                show_detail(&mut commands, facts, &server.0, &status, &font);
            }
            allow_join(&mut commands, &status);
        }
    }
}

/// Asks every listed server again on F5.
fn refresh_on_f5(
    keys: Res<ButtonInput<KeyCode>>,
    mut boards: Query<&mut StatusBoard>,
    cards: Query<&ServerCard>,
    probe: Res<StatusProbe>,
) {
    if !keys.just_pressed(KeyCode::F5) {
        return;
    }
    if let Ok(mut board) = boards.single_mut() {
        ping_every_server(&mut board, &cards, *probe);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::*;

    fn online(players: u32, max_players: u32, ms: u64) -> ServerStatus {
        ServerStatus::Online {
            players,
            max_players,
            latency: Duration::from_millis(ms),
        }
    }

    #[test]
    fn each_status_reads_as_a_word_and_a_colour() {
        assert_eq!(
            card_status(&online(3, 20, 42)),
            ("3 / 20".to_string(), TEXT)
        );
        assert_eq!(
            card_status(&ServerStatus::Pinging),
            ("Pinging...".to_string(), MUTED)
        );
        assert_eq!(card_status(&ServerStatus::Offline).1, MUTED);
        for status in [
            ServerStatus::Incompatible,
            ServerStatus::UnknownHost,
            ServerStatus::BadAddress,
        ] {
            assert_eq!(card_status(&status).1, DANGER, "{status:?}");
        }
        let (headline, _, line) = pane_status(&online(3, 20, 42));
        assert!(headline.contains("3 / 20 players"), "{headline}");
        assert_eq!(line, "42 ms");
    }

    /// Held by the test while the slow probe must not answer.
    static SLOW_GATE: Mutex<()> = Mutex::new(());
    /// Set once the slow probe has answered.
    static SLOW_DONE: AtomicBool = AtomicBool::new(false);

    /// Says offline once the test opens the gate.
    fn slow_offline(_: &str) -> ServerStatus {
        drop(SLOW_GATE.lock());
        SLOW_DONE.store(true, Ordering::SeqCst);
        ServerStatus::Offline
    }

    fn instant_online(_: &str) -> ServerStatus {
        online(3, 20, 42)
    }

    #[test]
    fn an_address_edited_away_and_back_is_asked_afresh() {
        let mut board = StatusBoard::default();
        board.ping("10.0.0.4:8000", StatusProbe(instant_online));
        let start = Instant::now();
        while board.status("10.0.0.4:8000") == ServerStatus::Pinging {
            assert!(start.elapsed() < Duration::from_secs(5), "never answered");
            board.collect();
            thread::yield_now();
        }
        // as the list is rebuilt after each edit: A to B, then back to A typed another way
        board.keep_only(["10.0.0.9"].into_iter());
        assert!(!board.knows("10.0.0.4:8000"));
        board.keep_only(["10.0.0.4"].into_iter());
        assert!(!board.knows("10.0.0.4"));
        assert_eq!(board.status("10.0.0.4"), ServerStatus::Pinging);
    }

    #[test]
    fn a_refresh_drops_the_probes_in_flight() {
        let gate = SLOW_GATE.lock().unwrap();
        let mut board = StatusBoard::default();
        board.ping("a", StatusProbe(slow_offline));
        board.ping("a", StatusProbe(instant_online));
        let start = Instant::now();
        while board.status("a") == ServerStatus::Pinging {
            assert!(start.elapsed() < Duration::from_secs(5), "never answered");
            board.collect();
            thread::yield_now();
        }
        assert_eq!(board.status("a"), online(3, 20, 42));

        drop(gate);
        while !SLOW_DONE.load(Ordering::SeqCst) {
            assert!(start.elapsed() < Duration::from_secs(5), "never answered");
            thread::yield_now();
        }
        // the slow answer is written a moment after its probe returned: give it the time
        let after = Instant::now();
        while after.elapsed() < Duration::from_millis(100) {
            board.collect();
            assert_eq!(board.status("a"), online(3, 20, 42));
            thread::yield_now();
        }
    }
}
