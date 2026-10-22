//! The shape of a bolt of lightning and its strokes over time, drawn from the seed of the strike
//! so that every player sees the same: a main channel from the cloud base down to the struck
//! block, jagged at every scale, and branches leaving it downward, forking and thinning out.

use bevy::prelude::*;

use crate::world::clouds::ALTITUDE;

/// Where a bolt leaves the clouds: a little inside their base.
pub const CLOUD_BASE: f32 = ALTITUDE as f32 + 1.;

/// The shortest a bolt is, from where it leaves the clouds to the block it strikes, for a block
/// as high as the clouds.
const MIN_HEIGHT: f32 = 24.;

/// How far the top of the main channel may lean away from the struck block, as a part of its
/// height.
const LEAN: [f32; 2] = [0.04, 0.28];

/// How far the middle of a piece of channel is thrown sideways, as a part of its length, for
/// the long pieces, those of a few blocks, and the short ones: the bolt wanders a little, turns
/// sharply every few blocks, now and then more sharply still (`KINK`, `KINKS`), and runs
/// nearly straight between its turns.
const ROUGHNESS: [f32; 3] = [0.17, 0.34, 0.09];
/// The lengths of piece, in blocks, at which the roughness turns from that of the long pieces
/// to that of a few blocks, and from that to that of the short ones.
const TURNS: [f32; 2] = [36., 3.5];

/// The longest a piece of channel is drawn: shorter pieces would not show.
const FINEST: f32 = 0.7;
const KINK: f32 = 2.2;
const KINKS: f32 = 0.12;

/// The width of the hot core of the main channel, in blocks, and the part of the width of their
/// parent the branches start with.
pub const MAIN_WIDTH: f32 = 0.3;
const BRANCH_WIDTH: f32 = 0.55;

/// A small generator of random numbers of our own (SplitMix64), so that a seed draws the same
/// bolt whatever the build, on every client.
pub struct BoltRng(u64);

impl BoltRng {
    pub fn new(seed: u32) -> Self {
        Self(u64::from(seed) ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, [from, to]: [f32; 2]) -> f32 {
        from + (to - from) * self.unit()
    }

    /// A number of a normal distribution, of mean 0 and deviation 1.
    fn normal(&mut self) -> f32 {
        let (a, b) = (self.unit().max(1e-7), self.unit());
        (-2. * a.ln()).sqrt() * (std::f32::consts::TAU * b).cos()
    }
}

/// A point of a channel: where it is, the width of its core in blocks, and how bright it is, as
/// a part of the main channel's brightness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoltPoint {
    pub position: Vec3,
    pub width: f32,
    pub brightness: f32,
}

/// A channel of the bolt: the main one (depth 0), from the clouds to the ground, or a branch,
/// leaving its parent at its first point.
#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    pub points: Vec<BoltPoint>,
    pub depth: u32,
}

/// A bolt of lightning: its main channel first, then its branches, each after its parent.
#[derive(Clone, Debug, PartialEq)]
pub struct Bolt {
    pub branches: Vec<Branch>,
}

/// How far the middle of a piece `length` blocks long is thrown sideways (`ROUGHNESS`).
fn roughness(length: f32) -> f32 {
    let [long, few, short] = ROUGHNESS;
    let step =
        |from: f32, to: f32| ((length.ln() - from.ln()) / (to.ln() - from.ln())).clamp(0., 1.);
    let rough = long + (few - long) * step(TURNS[0], TURNS[0] * 0.5);
    rough + (short - rough) * step(TURNS[1], TURNS[1] * 0.5)
}

/// The points of a channel from `from` to `to`, by midpoint displacement: each piece is split
/// in two at a middle thrown sideways by a part of its length, until the pieces are short.
fn channel(rng: &mut BoltRng, from: Vec3, to: Vec3) -> Vec<Vec3> {
    let mut points = vec![from, to];
    loop {
        let mut split = Vec::with_capacity(points.len() * 2);
        let mut any = false;
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            split.push(a);
            let length = a.distance(b);
            if length > FINEST {
                any = true;
                let along = (b - a) / length;
                let (side, other) = along.any_orthonormal_pair();
                let roughness = roughness(length);
                let kink = if length > TURNS[1] && rng.unit() < KINKS {
                    KINK
                } else {
                    1.
                };
                let throw =
                    (side * rng.normal() + other * rng.normal()) * roughness * kink * length;
                // Hardly ever up: a bolt makes its way down
                split.push((a + b) / 2. + throw * Vec3::new(0.5, 0.15, 0.5));
            }
        }
        split.push(*points.last().unwrap());
        points = split;
        if !any {
            return points;
        }
    }
}

impl Bolt {
    /// The bolt of the strike of seed `seed` on the block at `struck`.
    pub fn new(struck: IVec3, seed: u32) -> Self {
        let mut rng = BoltRng::new(seed);
        let target = struck.as_vec3() + Vec3::new(0.5, 1., 0.5);
        let top_y = CLOUD_BASE.max(target.y + MIN_HEIGHT);
        let height = top_y - target.y;
        let lean =
            Vec2::from_angle(rng.range([0., std::f32::consts::TAU])) * rng.range(LEAN) * height;
        let top = Vec3::new(target.x + lean.x, top_y, target.z + lean.y);
        let main = channel(&mut rng, top, target);
        let mut branches = vec![Branch {
            points: main
                .iter()
                .map(|&position| BoltPoint {
                    position,
                    width: MAIN_WIDTH,
                    brightness: 1.,
                })
                .collect(),
            depth: 0,
        }];
        let count = 4 + (rng.unit() * 6.) as usize;
        for _ in 0..count {
            // More from the upper part of the channel, as a stepped leader forks on its way down
            let at = (rng.unit().powf(1.6) * 0.85 * main.len() as f32) as usize;
            grow(&mut rng, &mut branches, 0, at, target.y, 1);
        }
        Self { branches }
    }

    /// Every point of every channel.
    #[cfg(test)]
    pub fn points(&self) -> impl Iterator<Item = &BoltPoint> {
        self.branches.iter().flat_map(|branch| &branch.points)
    }
}

/// Grows a branch of depth `depth` from the point `at` of the channel `parent` of `branches`,
/// down toward `ground` but not to it, and the branches of that branch.
fn grow(
    rng: &mut BoltRng,
    branches: &mut Vec<Branch>,
    parent: usize,
    at: usize,
    ground: f32,
    depth: u32,
) {
    let points = &branches[parent].points;
    let at = at.min(points.len() - 2);
    let start = points[at];
    let parent_direction = (points[(at + 6).min(points.len() - 1)].position - start.position)
        .normalize_or(Vec3::NEG_Y);
    // Away from the parent, outward and down
    let outward = Vec2::from_angle(rng.range([0., std::f32::consts::TAU])).extend(0.);
    let outward = Vec3::new(outward.x, 0., outward.y);
    let angle = rng.range([0.3, 0.85]);
    let direction = (parent_direction * angle.cos() + outward * angle.sin()).normalize();
    let direction = direction.with_y(direction.y.min(-0.5)).normalize();
    let room = (start.position.y - ground).max(2.);
    let length = room * rng.range([0.18, 0.5]) / (depth as f32).sqrt();
    let end = start.position + direction * length;
    let end = end.with_y(end.y.max(ground + 1.));
    let shape = channel(rng, start.position, end);
    let last = (shape.len() - 1) as f32;
    let width = start.width * BRANCH_WIDTH;
    let brightness = start.brightness * rng.range([0.45, 0.65]);
    let branch = Branch {
        points: shape
            .iter()
            .enumerate()
            .map(|(i, &position)| {
                let along = i as f32 / last;
                BoltPoint {
                    position,
                    // Thinning and fading to their tips
                    width: width * (1. - 0.75 * along),
                    brightness: brightness * (1. - 0.85 * along * along),
                }
            })
            .collect(),
        depth,
    };
    let length = branch.points.len();
    branches.push(branch);
    let index = branches.len() - 1;
    if depth >= 3 || length < 8 {
        return;
    }
    let forks = (rng.unit() * (4 - depth) as f32) as usize;
    for _ in 0..forks {
        let at = (rng.range([0.1, 0.7]) * length as f32) as usize;
        grow(rng, branches, index, at, ground, depth + 1);
    }
}

/// The strokes of a strike over time: two or three return strokes that light the channel up,
/// each a sharp rise and a quick decay, the first the brightest; between them the channel
/// glows faintly. Only the first stroke lights the branches: the later ones follow the main
/// channel alone, as real ones do.
#[derive(Clone, Debug, PartialEq)]
pub struct Strokes {
    /// When each stroke peaks, in seconds after the strike, and how bright it is.
    peaks: Vec<(f32, f32)>,
}

/// How long a stroke takes to rise, and how fast it decays, on the main channel and on the
/// branches, which go dark first.
const RISE: f32 = 0.012;
const DECAY: f32 = 0.04;
const BRANCH_DECAY: f32 = 0.026;
/// How bright the channel glows between the strokes, and how long it lingers after the last.
const GLOW: f32 = 0.12;
const LINGER: f32 = 0.16;

impl Strokes {
    pub fn new(seed: u32) -> Self {
        // A generator apart from the bolt's: the strokes do not depend on the shape
        let mut rng = BoltRng::new(seed.rotate_left(16) ^ 0x5bd1_e995);
        let mut peaks = vec![(RISE, 1.)];
        let count = if rng.unit() < 0.5 { 2 } else { 3 };
        let mut at = RISE;
        for _ in 1..count {
            at += rng.range([0.09, 0.17]);
            peaks.push((at, rng.range([0.6, 0.88])));
        }
        Self { peaks }
    }

    /// How long the strike lasts, in seconds.
    pub fn life(&self) -> f32 {
        self.peaks.last().unwrap().0 + LINGER
    }

    /// How bright one stroke peaking at `peak` is at `t`, decaying by `decay`.
    fn stroke(t: f32, (peak, brightness): (f32, f32), decay: f32) -> f32 {
        let start = peak - RISE;
        if t < start {
            0.
        } else if t < peak {
            let x = (t - start) / RISE;
            brightness * x * x * (3. - 2. * x)
        } else {
            brightness * (-(t - peak) / decay).exp()
        }
    }

    /// How bright the main channel is at `t` seconds after the strike, from 0 to 1: 1 at the
    /// peak of the first stroke.
    pub fn channel(&self, t: f32) -> f32 {
        if t < 0. || t > self.life() {
            return 0.;
        }
        let strokes = self
            .peaks
            .iter()
            .map(|&peak| Self::stroke(t, peak, DECAY))
            .fold(0., f32::max);
        let last = self.peaks.last().unwrap().0;
        let fade = 1. - ((t - last) / LINGER).clamp(0., 1.);
        let glow = GLOW * (t / RISE).min(1.) * fade * fade;
        strokes.max(glow).min(1.)
    }

    /// How bright the branches are at `t`, as a part of their own brightness: lit by the first
    /// stroke only, and fading faster.
    pub fn branches(&self, t: f32) -> f32 {
        if t < 0. || t > self.life() {
            return 0.;
        }
        let glow = GLOW * 0.5 * (1. - t / self.peaks[1].0).max(0.);
        Self::stroke(t, self.peaks[0], BRANCH_DECAY).max(glow)
    }

    /// When each stroke peaks, in seconds after the strike.
    #[cfg(test)]
    pub fn peaks(&self) -> impl Iterator<Item = f32> + '_ {
        self.peaks.iter().map(|&(at, _)| at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STRUCK: IVec3 = IVec3::new(40, 70, -12);

    fn main_channel(bolt: &Bolt) -> &[BoltPoint] {
        &bolt.branches[0].points
    }

    /// The most points a bolt holds: its pass draws six vertices for each. The tallest bolts,
    /// on a block far under the clouds, hold up to about 5,900 (over 2,000 seeds).
    const MOST_POINTS: usize = 8_192;

    #[test]
    fn a_bolt_holds_a_bounded_number_of_points_whatever_its_seed() {
        for y in [-64, 0, 62, 150, 250] {
            for seed in (0..400).map(|n| n * 7919 + 13) {
                let points = Bolt::new(IVec3::new(7, y, -3), seed).points().count();
                assert!(points <= MOST_POINTS, "{points} at {y} for {seed}");
            }
        }
        // the ones of an ordinary strike, far fewer
        let points = Bolt::new(STRUCK, 967).points().count();
        assert!(points < MOST_POINTS / 2, "{points}");
    }

    #[test]
    fn the_same_seed_draws_the_same_bolt_and_another_seed_another() {
        assert_eq!(Bolt::new(STRUCK, 7), Bolt::new(STRUCK, 7));
        assert_ne!(Bolt::new(STRUCK, 7), Bolt::new(STRUCK, 8));
        assert_eq!(Strokes::new(7), Strokes::new(7));
        // the generator itself, which every client must share
        let mut rng = BoltRng::new(1);
        let drawn: Vec<u32> = (0..3).map(|_| (rng.unit() * 1000.) as u32).collect();
        assert_eq!(drawn, [912, 219, 426]);
    }

    #[test]
    fn the_main_channel_runs_from_the_cloud_base_to_the_top_of_the_struck_block() {
        for seed in 0..50 {
            let bolt = Bolt::new(STRUCK, seed);
            let main = main_channel(&bolt);
            let target = Vec3::new(40.5, 71., -11.5);
            assert_eq!(main.first().unwrap().position.y, CLOUD_BASE);
            assert_eq!(main.last().unwrap().position, target);
            let height = CLOUD_BASE - target.y;
            for point in main {
                let side = point.position.xz().distance(target.xz());
                assert!(side < 0.75 * height, "{seed}: {point:?}");
                assert!(point.position.y < CLOUD_BASE + 0.3 * height, "{seed}");
                assert!(point.position.y > target.y - 0.3 * height, "{seed}");
            }
        }
        // a block as high as the clouds is still struck from above
        let high = Bolt::new(IVec3::new(0, 200, 0), 3);
        let top = main_channel(&high)[0].position.y;
        assert_eq!(top, 201. + MIN_HEIGHT);
    }

    #[test]
    fn the_main_channel_is_jagged_at_every_scale() {
        for seed in 0..20 {
            let bolt = Bolt::new(STRUCK, seed);
            let main = main_channel(&bolt);
            assert!(main.len() > 100, "{}", main.len());
            for pair in main.windows(2) {
                let length = pair[0].position.distance(pair[1].position);
                assert!(length <= FINEST * 1.5, "{seed}: {length}");
            }
            // far from the straight line between its ends
            let (top, bottom) = (main[0].position, main.last().unwrap().position);
            let line = (bottom - top).normalize();
            let off = |p: Vec3| (p - top).reject_from_normalized(line).length();
            let widest = main.iter().map(|p| off(p.position)).fold(0., f32::max);
            assert!(widest > 3., "{seed}: {widest}");
            // and turning sharply between pieces a few blocks long
            let turns = main
                .windows(9)
                .step_by(8)
                .collect::<Vec<_>>()
                .windows(2)
                .filter(|pair| {
                    let a = pair[0][8].position - pair[0][0].position;
                    let b = pair[1][8].position - pair[1][0].position;
                    a.angle_between(b) > 0.5
                })
                .count();
            assert!(turns >= 3, "{seed}: {turns}");
        }
    }

    #[test]
    fn branches_leave_their_parent_downward_thinner_and_dimmer() {
        for seed in 0..50 {
            let bolt = Bolt::new(STRUCK, seed);
            assert!((5..=60).contains(&bolt.branches.len()), "{seed}");
            assert!(
                bolt.branches.iter().any(|branch| branch.depth >= 2),
                "{seed}"
            );
            for (i, branch) in bolt.branches.iter().enumerate().skip(1) {
                let start = branch.points[0];
                let parent = bolt.branches[..i]
                    .iter()
                    .filter(|parent| parent.depth + 1 == branch.depth)
                    .flat_map(|parent| &parent.points)
                    .find(|point| point.position == start.position)
                    .unwrap_or_else(|| panic!("{seed}: branch {i} leaves no parent"));
                assert!(start.width < parent.width && start.brightness < parent.brightness);
                let end = branch.points.last().unwrap();
                assert!(end.position.y < start.position.y, "{seed}");
                assert!(end.position.y > 70., "{seed}: below the ground");
                assert!(end.width < start.width && end.brightness < start.brightness);
            }
        }
    }

    #[test]
    fn a_strike_flickers_in_two_or_three_strokes_the_first_the_brightest() {
        let mut counts = [0; 4];
        for seed in 0..200 {
            let strokes = Strokes::new(seed);
            assert_eq!(strokes.channel(-0.01), 0.);
            assert_eq!(strokes.channel(strokes.life() + 0.01), 0.);
            assert!(strokes.life() > 0.25 && strokes.life() < 0.6, "{seed}");
            let peaks: Vec<f32> = strokes.peaks().collect();
            counts[peaks.len()] += 1;
            assert_eq!(strokes.channel(peaks[0]), 1.);
            for pair in peaks.windows(2) {
                let (a, b) = (strokes.channel(pair[0]), strokes.channel(pair[1]));
                assert!(b < a.max(0.9) && b > 0.5, "{seed}: {b}");
                // dark between, but for a faint glow
                let between = strokes.channel(pair[1] - RISE - 0.005);
                assert!(between > 0. && between < 0.4, "{seed}: {between}");
            }
        }
        assert_eq!(counts[0] + counts[1], 0);
        assert!(counts[2] > 50 && counts[3] > 50, "{counts:?}");
    }

    /// Draws a few bolts seen from the side into the SVG file `BOLT_SVG`, to look at.
    #[test]
    #[ignore = "a picture for a human: BOLT_SVG=bolts.svg cargo test -p client draw_bolts"]
    fn draw_bolts() {
        let path = std::env::var("BOLT_SVG").unwrap();
        let mut svg = String::from(
            "<svg xmlns='http://www.w3.org/2000/svg' width='1600' height='700'><rect width='1600' height='700' fill='#0b0d14'/>",
        );
        for (i, seed) in [1u32, 2, 3, 4, 5, 6].into_iter().enumerate() {
            let bolt = Bolt::new(IVec3::new(0, 64, 0), seed * 7919);
            for branch in &bolt.branches {
                for pair in branch.points.windows(2) {
                    let at = |p: &BoltPoint| {
                        (
                            130. + i as f32 * 265. + p.position.x * 4.6,
                            680. - (p.position.y - 64.) * 4.6,
                        )
                    };
                    let ((x1, y1), (x2, y2)) = (at(&pair[0]), at(&pair[1]));
                    let width = pair[0].width * 6.;
                    let alpha = pair[0].brightness;
                    svg += &format!(
                        "<line x1='{x1}' y1='{y1}' x2='{x2}' y2='{y2}' stroke='rgb(225,215,255)' stroke-opacity='{alpha}' stroke-width='{width}' stroke-linecap='round'/>"
                    );
                }
            }
        }
        svg += "</svg>";
        std::fs::write(path, svg).unwrap();
    }

    #[test]
    fn the_branches_light_up_with_the_first_stroke_only() {
        let strokes = Strokes::new(11);
        let peaks: Vec<f32> = strokes.peaks().collect();
        assert_eq!(strokes.branches(peaks[0]), 1.);
        assert!(strokes.branches(peaks[1]) < 0.04);
        assert!(strokes.channel(peaks[1]) > 0.5);
        assert_eq!(strokes.branches(strokes.life() + 0.1), 0.);
    }
}
