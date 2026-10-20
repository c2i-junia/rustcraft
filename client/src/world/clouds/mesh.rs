use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, MeshVertexAttribute, PrimitiveTopology, VertexFormat},
    prelude::*,
};
use serde::{Deserialize, Serialize};

use super::{
    field::is_full, ALTITUDE, CELL, SHADE_BOTTOM, SHADE_EAST_WEST, SHADE_NORTH_SOUTH, SHADE_TOP,
    THICKNESS,
};

/// How the clouds are drawn, a video setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Reflect)]
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

/// An axis-aligned rectangle of a cloud, in blocks of the pattern: `min` and `max` are equal
/// along the axis of `normal`, which points out of the cloud.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub min: Vec3,
    pub max: Vec3,
    pub normal: IVec3,
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

/// The faces of the cells within `radius` cells of `centre` (a disk) for which `full` holds,
/// drawn in `style`.
pub fn quads(
    style: CloudStyle,
    centre: IVec2,
    radius: i32,
    full: impl Fn(IVec2) -> bool,
) -> Vec<Quad> {
    if style == CloudStyle::Off {
        return Vec::new();
    }
    let in_window = |cell: IVec2| (cell - centre).length_squared() <= radius * radius;
    let full = |cell: IVec2| in_window(cell) && full(cell);
    let bottom = ALTITUDE as f32;
    let top = (ALTITUDE + THICKNESS) as f32;
    let block = |cells: i32| (cells * CELL) as f32;

    let mut quads = Vec::new();
    for z in centre.y - radius..=centre.y + radius {
        let mut x = centre.x - radius;
        while x <= centre.x + radius {
            if !full(IVec2::new(x, z)) {
                x += 1;
                continue;
            }
            let start = x;
            while full(IVec2::new(x, z)) {
                x += 1;
            }
            // the run spans the cells `start..x` of row `z`
            let (west, east) = (block(start), block(x));
            let (north, south) = (block(z), block(z + 1));
            if style == CloudStyle::Fast {
                let middle = ALTITUDE as f32 + THICKNESS as f32 / 2.;
                quads.push(Quad {
                    min: Vec3::new(west, middle, north),
                    max: Vec3::new(east, middle, south),
                    normal: IVec3::Y,
                });
                continue;
            }
            quads.push(Quad {
                min: Vec3::new(west, top, north),
                max: Vec3::new(east, top, south),
                normal: IVec3::Y,
            });
            quads.push(Quad {
                min: Vec3::new(west, bottom, north),
                max: Vec3::new(east, bottom, south),
                normal: IVec3::NEG_Y,
            });
            quads.push(Quad {
                min: Vec3::new(west, bottom, north),
                max: Vec3::new(west, top, south),
                normal: IVec3::NEG_X,
            });
            quads.push(Quad {
                min: Vec3::new(east, bottom, north),
                max: Vec3::new(east, top, south),
                normal: IVec3::X,
            });
            // the sides along the run, merged where consecutive cells show them
            for (dz, side, normal) in [(-1, north, IVec3::NEG_Z), (1, south, IVec3::Z)] {
                let mut cell = start;
                while cell < x {
                    if full(IVec2::new(cell, z + dz)) {
                        cell += 1;
                        continue;
                    }
                    let from = cell;
                    while cell < x && !full(IVec2::new(cell, z + dz)) {
                        cell += 1;
                    }
                    quads.push(Quad {
                        min: Vec3::new(block(from), bottom, side),
                        max: Vec3::new(block(cell), top, side),
                        normal,
                    });
                }
            }
        }
    }
    quads
}

/// The mesh of the clouds within `radius` cells of `centre`, in blocks of the pattern: cell `c`
/// spans the blocks `c * CELL` to `(c + 1) * CELL`. `None` when there is nothing to draw.
pub fn cloud_mesh(style: CloudStyle, centre: IVec2, radius: i32) -> Option<Mesh> {
    let quads = quads(style, centre, radius, is_full);
    if quads.is_empty() {
        return None;
    }
    let positions: Vec<[f32; 3]> = quads
        .iter()
        .flat_map(|quad| quad.corners().map(<[f32; 3]>::from))
        .collect();
    let shades: Vec<f32> = quads.iter().flat_map(|quad| [quad.shade(); 4]).collect();
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
        .with_inserted_indices(Indices::U32(indices)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn fancy(cells: &[IVec2]) -> Vec<Quad> {
        quads(CloudStyle::Fancy, IVec2::ZERO, 8, |c| cells.contains(&c))
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
        let quads = quads(CloudStyle::Fast, IVec2::ZERO, 8, |c| cells.contains(&c));
        assert_eq!(quads.len(), 3);
        let middle = (ALTITUDE as f32) + THICKNESS as f32 / 2.;
        assert!(quads
            .iter()
            .all(|q| q.normal == IVec3::Y && q.min.y == middle && q.max.y == middle));
    }

    #[test]
    fn off_draws_nothing() {
        assert!(quads(CloudStyle::Off, IVec2::ZERO, 8, |_| true).is_empty());
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
        let quads = quads(CloudStyle::Fast, centre, 3, |_| true);
        // rows of 1, 5, 5, 7, 5, 5, 1 cells
        assert_eq!(quads.len(), 7);
        let cells: f32 = quads
            .iter()
            .map(|q| (q.max.x - q.min.x) / CELL as f32)
            .sum();
        assert_eq!(cells, 29.);
        // a full disk is closed on its edge
        let fancy = super::quads(CloudStyle::Fancy, centre, 3, |_| true);
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
}
