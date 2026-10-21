use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, MeshVertexAttribute, PrimitiveTopology, VertexFormat},
    prelude::*,
};
use serde::{Deserialize, Serialize};

use super::{
    field::{banded_density, NEVER},
    ALTITUDE, CELL, SHADE_BOTTOM, SHADE_EAST_WEST, SHADE_NORTH_SOUTH, SHADE_TOP, THICKNESS,
};

/// How the clouds are drawn, a video setting saved in `video.ron`.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Reflect)]
pub enum CloudStyle {
    /// Slabs of blocks, with sides.
    #[default]
    Fancy,
    /// A flat layer at the middle height of the slabs.
    Fast,
    /// No clouds.
    Off,
}

/// The brightness of a vertex, a fraction of the tint: each face of a cloud has its own.
pub const ATTRIBUTE_SHADE: MeshVertexAttribute =
    MeshVertexAttribute::new("CloudShade", 0x00C1_0D5A_DE00_0001, VertexFormat::Float32);

/// The densities of the two cells a face lies between, the denser first, which the shader
/// compares with the threshold of the weather: the face shows when only the denser one is
/// full.
pub const ATTRIBUTE_DENSITIES: MeshVertexAttribute = MeshVertexAttribute::new(
    "CloudDensities",
    0x00C1_0D5A_DE00_0002,
    VertexFormat::Float32x2,
);

/// An axis-aligned rectangle of a cloud, in blocks of the pattern: `min` and `max` are equal
/// along the axis of `normal`, which points out of the cloud.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub min: Vec3,
    pub max: Vec3,
    pub normal: IVec3,
    /// The density of the cell the face belongs to, and of the cell it looks into: `NEVER`
    /// for the sky above and below. The face shows at a threshold `t` when
    /// `low <= t < high`.
    pub high: f32,
    pub low: f32,
}

impl Quad {
    /// The brightness of the face, after the direction it looks to.
    pub fn shade(&self) -> f32 {
        match self.normal {
            IVec3 { y: 1, .. } => SHADE_TOP,
            IVec3 { y: -1, .. } => SHADE_BOTTOM,
            IVec3 { x: 1 | -1, .. } => SHADE_EAST_WEST,
            _ => SHADE_NORTH_SOUTH,
        }
    }

    /// Whether the face shows when the cells denser than `threshold` are full.
    pub fn shows_at(&self, threshold: f32) -> bool {
        self.low <= threshold && threshold < self.high
    }

    /// The four corners, counter-clockwise seen from the side `normal` points to.
    pub fn corners(&self) -> [Vec3; 4] {
        // two edges whose cross product is the normal, so that `min`, `min + u`,
        // `min + u + v`, `min + v` turn counter-clockwise around it
        let size = self.max - self.min;
        let (u, v) = match self.normal.abs() {
            IVec3::X => (Vec3::Y * size.y, Vec3::Z * size.z),
            IVec3::Y => (Vec3::Z * size.z, Vec3::X * size.x),
            _ => (Vec3::X * size.x, Vec3::Y * size.y),
        };
        let (u, v) = if self.normal.element_sum() < 0 {
            (v, u)
        } else {
            (u, v)
        };
        [self.min, self.min + u, self.min + u + v, self.min + v]
    }
}

/// The faces of the cells within `radius` cells of `centre` (a disk), with the density of
/// each cell given by `density` (`NEVER` for an empty one), drawn in `style`: every face that
/// shows at some threshold, between a cell and its neighbour or the sky. Neighbouring faces
/// that look the same way between cells of the same densities are merged.
pub fn quads(
    style: CloudStyle,
    centre: IVec2,
    radius: i32,
    density: impl Fn(IVec2) -> f32,
) -> Vec<Quad> {
    if style == CloudStyle::Off {
        return Vec::new();
    }
    // the densities of the window and of a ring of cells around it, read once
    let side = 2 * radius + 3;
    let corner = centre - IVec2::splat(radius + 1);
    let densities: Vec<f32> = (0..side * side)
        .map(|i| {
            let cell = corner + IVec2::new(i % side, i / side);
            if (cell - centre).length_squared() <= radius * radius {
                density(cell)
            } else {
                NEVER
            }
        })
        .collect();
    let density = |cell: IVec2| {
        let at = cell - corner;
        densities[(at.y * side + at.x) as usize]
    };
    let bottom = ALTITUDE as f32;
    let top = (ALTITUDE + THICKNESS) as f32;
    let block = |cells: i32| (cells * CELL) as f32;
    let (west, east) = (centre.x - radius, centre.x + radius);

    let mut quads = Vec::new();
    for z in centre.y - radius..=centre.y + radius {
        let (north, south) = (block(z), block(z + 1));
        // the tops and bottoms, along runs of cells of the same density
        let mut x = west;
        while x <= east {
            let high = density(IVec2::new(x, z));
            let start = x;
            while x <= east && density(IVec2::new(x, z)) == high {
                x += 1;
            }
            if high == NEVER {
                continue;
            }
            let (min_x, max_x) = (block(start), block(x));
            let horizontal = |y, normal| Quad {
                min: Vec3::new(min_x, y, north),
                max: Vec3::new(max_x, y, south),
                normal,
                high,
                low: NEVER,
            };
            if style == CloudStyle::Fast {
                let middle = ALTITUDE as f32 + THICKNESS as f32 / 2.;
                quads.push(horizontal(middle, IVec3::Y));
            } else {
                quads.push(horizontal(top, IVec3::Y));
                quads.push(horizontal(bottom, IVec3::NEG_Y));
            }
        }
        if style == CloudStyle::Fast {
            continue;
        }
        // the sides between a cell and the next one along +X
        for x in west - 1..=east {
            let (a, b) = (density(IVec2::new(x, z)), density(IVec2::new(x + 1, z)));
            if a == b {
                continue;
            }
            let at = block(x + 1);
            quads.push(Quad {
                min: Vec3::new(at, bottom, north),
                max: Vec3::new(at, top, south),
                normal: if a > b { IVec3::X } else { IVec3::NEG_X },
                high: a.max(b),
                low: a.min(b),
            });
        }
    }
    if style == CloudStyle::Fancy {
        // the sides between a cell and the next one along +Z, merged along X
        for z in centre.y - radius - 1..=centre.y + radius {
            let at = block(z + 1);
            let pair = |x| (density(IVec2::new(x, z)), density(IVec2::new(x, z + 1)));
            let mut x = west;
            while x <= east {
                let (a, b) = pair(x);
                let start = x;
                while x <= east && pair(x) == (a, b) {
                    x += 1;
                }
                if a == b {
                    continue;
                }
                quads.push(Quad {
                    min: Vec3::new(block(start), bottom, at),
                    max: Vec3::new(block(x), top, at),
                    normal: if a > b { IVec3::Z } else { IVec3::NEG_Z },
                    high: a.max(b),
                    low: a.min(b),
                });
            }
        }
    }
    quads
}

/// The mesh of the clouds within `radius` cells of `centre`, in blocks of the pattern: cell `c`
/// spans the blocks `c * CELL` to `(c + 1) * CELL`. It holds every cell full at some threshold
/// of the weather, which the shader picks among. `None` when there is nothing to draw.
pub fn cloud_mesh(style: CloudStyle, centre: IVec2, radius: i32) -> Option<Mesh> {
    let quads = quads(style, centre, radius, banded_density);
    if quads.is_empty() {
        return None;
    }
    let positions: Vec<[f32; 3]> = quads
        .iter()
        .flat_map(|quad| quad.corners().map(<[f32; 3]>::from))
        .collect();
    let shades: Vec<f32> = quads.iter().flat_map(|quad| [quad.shade(); 4]).collect();
    let densities: Vec<[f32; 2]> = quads
        .iter()
        .flat_map(|quad| [[quad.high, quad.low]; 4])
        .collect();
    let indices: Vec<u32> = (0..quads.len() as u32)
        .flat_map(|quad| [0, 1, 2, 0, 2, 3].map(|corner| quad * 4 + corner))
        .collect();
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(ATTRIBUTE_SHADE, shades)
        .with_inserted_attribute(ATTRIBUTE_DENSITIES, densities)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

#[cfg(test)]
mod tests {
    use super::super::field::{
        density, is_full, ALWAYS, CLEAR_THRESHOLD, RAIN_THRESHOLD, STORM_THRESHOLD,
    };
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    /// The density of a cell that is full at every threshold if one of `cells`, else empty.
    fn full_in(cells: &[IVec2], cell: IVec2) -> f32 {
        if cells.contains(&cell) {
            ALWAYS
        } else {
            NEVER
        }
    }

    fn fancy(cells: &[IVec2]) -> Vec<Quad> {
        quads(CloudStyle::Fancy, IVec2::ZERO, 8, |c| full_in(cells, c))
    }

    fn count(quads: &[Quad], normal: IVec3) -> usize {
        quads.iter().filter(|q| q.normal == normal).count()
    }

    const NORMALS: [IVec3; 6] = [
        IVec3::Y,
        IVec3::NEG_Y,
        IVec3::X,
        IVec3::NEG_X,
        IVec3::Z,
        IVec3::NEG_Z,
    ];

    #[test]
    fn an_isolated_cell_has_six_faces() {
        let quads = fancy(&[IVec2::new(2, -3)]);
        assert_eq!(quads.len(), 6);
        for normal in NORMALS {
            assert_eq!(count(&quads, normal), 1, "{normal}");
        }
        let top = quads.iter().find(|q| q.normal == IVec3::Y).unwrap();
        assert_eq!(top.min, Vec3::new(24., (ALTITUDE + THICKNESS) as f32, -36.));
        assert_eq!(top.max, Vec3::new(36., (ALTITUDE + THICKNESS) as f32, -24.));
        let bottom = quads.iter().find(|q| q.normal == IVec3::NEG_Y).unwrap();
        assert_eq!(bottom.min.y, ALTITUDE as f32);
    }

    #[test]
    fn two_neighbours_share_no_face() {
        // along Z: rows are merged only along X, so each cell keeps its top and bottom
        let quads = fancy(&[IVec2::new(0, 0), IVec2::new(0, 1)]);
        assert_eq!(quads.len(), 10);
        assert_eq!(count(&quads, IVec3::Z), 1);
        assert_eq!(count(&quads, IVec3::NEG_Z), 1);
        let between = CELL as f32;
        assert!(!quads.iter().any(|q| q.normal.z != 0 && q.min.z == between));
    }

    #[test]
    fn a_row_of_cells_has_one_top_and_one_bottom() {
        let row: Vec<IVec2> = (-3..4).map(|x| IVec2::new(x, 1)).collect();
        let quads = fancy(&row);
        assert_eq!(quads.len(), 6);
        let top = quads.iter().find(|q| q.normal == IVec3::Y).unwrap();
        assert_eq!(top.min.x, (-3 * CELL) as f32);
        assert_eq!(top.max.x, (4 * CELL) as f32);
    }

    #[test]
    fn fast_clouds_are_one_quad_per_run() {
        // two runs in a row, one in another
        let cells = [
            IVec2::new(0, 0),
            IVec2::new(1, 0),
            IVec2::new(3, 0),
            IVec2::new(0, 2),
        ];
        let quads = quads(CloudStyle::Fast, IVec2::ZERO, 8, |c| full_in(&cells, c));
        assert_eq!(quads.len(), 3);
        let middle = (ALTITUDE as f32) + THICKNESS as f32 / 2.;
        assert!(quads
            .iter()
            .all(|q| q.normal == IVec3::Y && q.min.y == middle && q.max.y == middle));
    }

    #[test]
    fn off_draws_nothing() {
        assert!(quads(CloudStyle::Off, IVec2::ZERO, 8, |_| ALWAYS).is_empty());
        assert!(cloud_mesh(CloudStyle::Off, IVec2::ZERO, 8).is_none());
    }

    #[test]
    fn each_face_has_the_shade_of_its_orientation() {
        let quads = fancy(&[IVec2::ZERO]);
        let shade = |normal| quads.iter().find(|q| q.normal == normal).unwrap().shade();
        assert_eq!(shade(IVec3::Y), SHADE_TOP);
        assert_eq!(shade(IVec3::NEG_Y), SHADE_BOTTOM);
        assert_eq!(shade(IVec3::Z), SHADE_NORTH_SOUTH);
        assert_eq!(shade(IVec3::NEG_Z), SHADE_NORTH_SOUTH);
        assert_eq!(shade(IVec3::X), SHADE_EAST_WEST);
        assert_eq!(shade(IVec3::NEG_X), SHADE_EAST_WEST);
    }

    #[test]
    fn every_face_is_counter_clockwise_seen_from_outside() {
        let cells = [IVec2::new(0, 0), IVec2::new(1, 0), IVec2::new(1, 1)];
        for quad in fancy(&cells) {
            let [a, b, c, d] = quad.corners();
            let normal = quad.normal.as_vec3();
            assert!((b - a).cross(c - a).dot(normal) > 0., "{quad:?}");
            assert!((c - a).cross(d - a).dot(normal) > 0., "{quad:?}");
            // the corners span the rectangle
            assert_eq!([a, b, c, d].into_iter().reduce(Vec3::min), Some(quad.min));
            assert_eq!([a, b, c, d].into_iter().reduce(Vec3::max), Some(quad.max));
        }
    }

    #[test]
    fn only_the_cells_of_the_disk_are_drawn() {
        let centre = IVec2::new(100, -50);
        let quads = quads(CloudStyle::Fast, centre, 3, |_| ALWAYS);
        // rows of 1, 5, 5, 7, 5, 5, 1 cells
        assert_eq!(quads.len(), 7);
        let cells: f32 = quads
            .iter()
            .map(|q| (q.max.x - q.min.x) / CELL as f32)
            .sum();
        assert_eq!(cells, 29.);
        // a full disk is closed on its edge
        let fancy = super::quads(CloudStyle::Fancy, centre, 3, |_| ALWAYS);
        assert_eq!(count(&fancy, IVec3::X), 7);
        assert_eq!(count(&fancy, IVec3::NEG_X), 7);
    }

    #[test]
    fn the_mesh_has_a_shade_for_each_position() {
        let mesh = cloud_mesh(CloudStyle::Fancy, IVec2::ZERO, 20).expect("some clouds");
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len();
        let Some(VertexAttributeValues::Float32(shades)) = mesh.attribute(ATTRIBUTE_SHADE) else {
            panic!("no shades");
        };
        assert!(positions > 0);
        assert_eq!(shades.len(), positions);
        assert_eq!(mesh.indices().unwrap().len() * 4, positions * 6);
        assert!(shades.iter().all(|s| [
            SHADE_TOP,
            SHADE_BOTTOM,
            SHADE_NORTH_SOUTH,
            SHADE_EAST_WEST
        ]
        .contains(s)));
    }

    #[test]
    fn the_mesh_has_the_densities_of_each_face() {
        let mesh = cloud_mesh(CloudStyle::Fancy, IVec2::ZERO, 20).expect("some clouds");
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len();
        let Some(VertexAttributeValues::Float32x2(densities)) = mesh.attribute(ATTRIBUTE_DENSITIES)
        else {
            panic!("no densities");
        };
        assert_eq!(densities.len(), positions);
        assert!(densities.iter().all(|[high, low]| high > low));
    }

    /// The area of the faces of `quads` that look along `axis`, in cells for the tops and
    /// bottoms and in cell sides for the sides.
    fn area(quads: &[Quad], normal: IVec3) -> f32 {
        let size = |q: &Quad| {
            let size = (q.max - q.min) / CELL as f32;
            match normal.abs() {
                IVec3::Y => size.x * size.z,
                IVec3::X => size.z,
                _ => size.x,
            }
        };
        quads.iter().filter(|q| q.normal == normal).map(size).sum()
    }

    #[test]
    fn at_any_threshold_a_face_shows_where_one_cell_is_full_and_the_next_one_is_not() {
        let (centre, radius) = (IVec2::new(37, -11), 12);
        let quads = quads(CloudStyle::Fancy, centre, radius, banded_density);
        for threshold in [
            CLEAR_THRESHOLD,
            -0.1,
            RAIN_THRESHOLD,
            -0.33,
            STORM_THRESHOLD,
        ] {
            let shown: Vec<Quad> = quads
                .iter()
                .filter(|q| q.shows_at(threshold))
                .copied()
                .collect();
            let full = |cell: IVec2| {
                (cell - centre).length_squared() <= radius * radius && density(cell) > threshold
            };
            let cells = || {
                (-radius - 1..=radius).flat_map(move |z| {
                    (-radius - 1..=radius).map(move |x| centre + IVec2::new(x, z))
                })
            };
            let full_cells = cells().filter(|c| full(*c)).count() as f32;
            assert_eq!(area(&shown, IVec3::Y), full_cells, "{threshold}");
            assert_eq!(area(&shown, IVec3::NEG_Y), full_cells, "{threshold}");
            for (step, normal) in [(IVec2::X, IVec3::X), (IVec2::Y, IVec3::Z)] {
                // full then empty: the face looks along the step, else back
                let out = cells().filter(|c| full(*c) && !full(*c + step)).count();
                let back = cells().filter(|c| !full(*c) && full(*c + step)).count();
                assert_eq!(area(&shown, normal), out as f32, "{threshold} {normal}");
                assert_eq!(area(&shown, -normal), back as f32, "{threshold} {normal}");
            }
        }
    }

    #[test]
    fn under_a_clear_sky_the_faces_are_those_of_the_full_cells() {
        let (centre, radius) = (IVec2::new(-5, 70), 10);
        let clear = |c| if is_full(c) { ALWAYS } else { NEVER };
        let before = quads(CloudStyle::Fancy, centre, radius, clear);
        let now: Vec<Quad> = quads(CloudStyle::Fancy, centre, radius, banded_density)
            .into_iter()
            .filter(|q| q.shows_at(CLEAR_THRESHOLD))
            .collect();
        for normal in NORMALS {
            assert_eq!(area(&now, normal), area(&before, normal), "{normal}");
        }
        // and the tops of the cells full at every threshold are merged as before
        assert_eq!(count(&now, IVec3::Y), count(&before, IVec3::Y));
        // the fast clouds too
        let before = quads(CloudStyle::Fast, centre, radius, clear);
        let now: Vec<Quad> = quads(CloudStyle::Fast, centre, radius, banded_density)
            .into_iter()
            .filter(|q| q.shows_at(CLEAR_THRESHOLD))
            .collect();
        assert_eq!(now.len(), before.len());
        assert_eq!(area(&now, IVec3::Y), area(&before, IVec3::Y));
    }

    #[test]
    fn a_storm_shows_more_of_the_mesh_than_rain_and_rain_more_than_a_clear_sky() {
        let quads = quads(CloudStyle::Fast, IVec2::ZERO, 30, banded_density);
        let shown = |threshold| {
            area(
                &quads
                    .iter()
                    .filter(|q| q.shows_at(threshold))
                    .copied()
                    .collect::<Vec<_>>(),
                IVec3::Y,
            )
        };
        let (clear, rain, storm) = (
            shown(CLEAR_THRESHOLD),
            shown(RAIN_THRESHOLD),
            shown(STORM_THRESHOLD),
        );
        assert!(clear < rain && rain < storm, "{clear} {rain} {storm}");
    }
}
