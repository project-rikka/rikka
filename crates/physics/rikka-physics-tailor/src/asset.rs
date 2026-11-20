use glam::{Mat4, Vec2, Vec3};
use rikka_model_core::NodeId;
use serde::{Deserialize, Serialize};

use crate::preset::Preset;
use crate::style::Style;

pub const MAX_INFLUENCES: usize = 3;
pub const MAX_STRAND_NODES: usize = 16;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClothAsset {
    pub rest_worlds: Vec<Mat4>,
    pub pieces: Vec<Piece>,
    pub body: Body,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PieceKey {
    pub node: NodeId,
    pub vertex: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PieceKind {
    Sheet,
    Strands,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub key: PieceKey,
    pub name: String,
    pub kind: PieceKind,
    pub preset: Preset,
    pub style: Style,
    pub layer: u8,
    pub particles: Vec<Particle>,
    pub edges: Vec<Edge>,
    pub bends: Vec<Bend>,
    pub tethers: Vec<Tether>,
    pub triangles: Vec<[u32; 3]>,
    pub strand_nodes: Vec<StrandNode>,
    pub bindings: Vec<VertexBinding>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub rest: Vec3,
    pub anchor: Anchor,
    pub pinned: bool,
    pub mass: f32,
    pub depth: f32,
    pub reach: f32,
    pub parent: Option<u32>,
    pub radius: f32,
    pub clearance: f32,
    pub render_offset: f32,
    pub relief_height: f32,
    pub relief_depth: f32,
    pub spread: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    Skinned {
        joints: [NodeId; 4],
        weights: [f32; 4],
    },
    Surface {
        triangle: u32,
        uv: Vec2,
        offset: Vec3,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Structural,
    Shear,
    Lateral,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub ends: [u32; 2],
    pub rest: f32,
    pub kind: EdgeKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bend {
    pub hinge: [u32; 2],
    pub wings: [u32; 2],
    pub rest_angle: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tether {
    pub particle: u32,
    pub anchor: u32,
    pub length: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrandNode {
    pub origin: u32,
    pub side: u32,
    pub along: [u32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexBinding {
    pub node: NodeId,
    pub vertex: u32,
    pub blend: f32,
    pub gap: f32,
    pub target: BindTarget,
    pub underlay: Option<Underlay>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Underlay {
    pub piece: u32,
    pub triangle: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BindTarget {
    Triangles([Influence; MAX_INFLUENCES]),
    Strand(StrandBinding),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Influence {
    pub triangle: u32,
    pub uv: Vec2,
    pub offset: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub weight: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StrandBinding {
    pub weights: [StrandWeight; MAX_STRAND_NODES],
    pub offset: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StrandWeight {
    pub strand_node: u32,
    pub weight: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Body {
    pub vertices: Vec<BodyVertex>,
    pub triangles: Vec<[u32; 3]>,
    pub colliders: Vec<BodyCollider>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyVertex {
    pub rest: Vec3,
    pub joints: [NodeId; 4],
    pub weights: [f32; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct BodyCollider {
    pub node: NodeId,
    pub shape: BodyShape,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BodyShape {
    Tapered { ends: [Vec3; 2], radii: [f32; 2] },
    Hull { planes: Vec<(Vec3, f32)> },
}

impl Bend {
    #[must_use]
    pub fn angle(&self, positions: &[Vec3]) -> f32 {
        let [h0, h1] = self.hinge.map(|end| positions[end as usize]);
        let [w0, w1] = self.wings.map(|end| positions[end as usize]);
        let axis = (h1 - h0).normalize_or_zero();
        let first = (h1 - h0).cross(w0 - h0).normalize_or_zero();
        let second = (w1 - h0).cross(h1 - h0).normalize_or_zero();
        first.cross(second).dot(axis).atan2(first.dot(second))
    }
}

impl BodyCollider {
    #[must_use]
    pub fn placed(&self, worlds: &[Mat4]) -> BodyShape {
        self.shape.placed(
            worlds
                .get(self.node.index())
                .copied()
                .unwrap_or(Mat4::IDENTITY),
        )
    }
}

impl BodyShape {
    #[must_use]
    pub fn distance(&self, point: Vec3) -> f32 {
        match self {
            BodyShape::Tapered { ends, radii } => tapered_distance(*ends, *radii, point),
            BodyShape::Hull { planes } => planes
                .iter()
                .map(|(normal, offset)| normal.dot(point) - offset)
                .fold(f32::MIN, f32::max),
        }
    }

    #[must_use]
    pub fn placed(&self, world: Mat4) -> BodyShape {
        let (_, rotation, translation) = world.to_scale_rotation_translation();
        let rotation = rotation.normalize();
        match self {
            BodyShape::Tapered { ends, radii } => BodyShape::Tapered {
                ends: ends.map(|end| rotation * end + translation),
                radii: *radii,
            },
            BodyShape::Hull { planes } => BodyShape::Hull {
                planes: planes
                    .iter()
                    .map(|(normal, offset)| {
                        let normal = rotation * *normal;
                        (normal, offset + normal.dot(translation))
                    })
                    .collect(),
            },
        }
    }
}

fn tapered_distance([a, b]: [Vec3; 2], [radius_a, radius_b]: [f32; 2], point: Vec3) -> f32 {
    let axis = b - a;
    let length = axis.length();
    let difference = radius_a - radius_b;
    let along = if length <= difference.abs() {
        if radius_a >= radius_b { 0.0 } else { 1.0 }
    } else {
        let offset = point - a;
        let straight = offset.dot(axis) / (length * length);
        let away = (offset - axis * straight).length();
        let sine = difference / length;
        let lean = sine / (1.0 - sine * sine).sqrt() / length;
        (straight - away * lean).clamp(0.0, 1.0)
    };
    point.distance(a + axis * along) - (radius_a - difference * along)
}
