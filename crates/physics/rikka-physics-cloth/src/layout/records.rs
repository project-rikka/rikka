use bytemuck::{Pod, Zeroable};
use rikka_physics_tailor::MAX_STRAND_NODES;

pub(crate) const NONE: u32 = u32::MAX;
pub(crate) const SURFACE: f32 = 1.0;
pub(crate) const CONTACT_WORDS: u64 = 24;
pub(crate) const GEOMETRY_VECS: u64 = 5 * 8;
pub(crate) const WRAP_WORDS: u64 = 4;
pub(crate) const NEAREST_WORDS: u64 = 9;
pub(crate) const STRAND: u32 = 1 << 31;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct AnchorRecord {
    pub(crate) ids: [u32; 4],
    pub(crate) weights: [f32; 4],
    pub(crate) extra: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct TraitRecord {
    pub(crate) a: [f32; 4],
    pub(crate) b: [f32; 4],
    pub(crate) c: [f32; 4],
    pub(crate) d: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct EdgeRecord {
    pub(crate) ends: [u32; 4],
    pub(crate) params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct HingeRecord {
    pub(crate) ids: [u32; 4],
    pub(crate) params: [f32; 4],
    pub(crate) weights: [f32; 4],
    pub(crate) applied: [f32; 4],
    pub(crate) dests: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct TetherRecord {
    pub(crate) anchor: u32,
    pub(crate) length: f32,
    pub(crate) shortest: f32,
    pub(crate) padding: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct InfluenceRecord {
    pub(crate) triangle: u32,
    pub(crate) place: [f32; 3],
    pub(crate) offset: [f32; 4],
    pub(crate) directions: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct TrianglesRecord {
    pub(crate) influences: [InfluenceRecord; 3],
    pub(crate) rest: [f32; 3],
    pub(crate) underlay: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct StrandRecord {
    pub(crate) strand_nodes: [u32; MAX_STRAND_NODES],
    pub(crate) weights: [f32; MAX_STRAND_NODES],
    pub(crate) offset: [f32; 4],
    pub(crate) directions: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct BindRecord {
    pub(crate) header: [u32; 4],
    pub(crate) target: [[u32; 4]; 10],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct StrandNodeRecord {
    pub(crate) ids: [u32; 4],
    pub(crate) tangent: [f32; 4],
    pub(crate) bitangent: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct PieceRecord {
    pub(crate) centre: [f32; 4],
    pub(crate) scaled: [f32; 4],
    pub(crate) turn: [f32; 4],
    pub(crate) fixed_shift: [f32; 4],
    pub(crate) spin: [f32; 4],
    pub(crate) response: [f32; 4],
    pub(crate) flutter: [f32; 4],
    pub(crate) contact: [f32; 4],
    pub(crate) settling: [f32; 4],
    pub(crate) layer_triangles: [u32; 4],
    pub(crate) grab: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct ColliderRecord {
    pub(crate) start: [f32; 4],
    pub(crate) end: [f32; 4],
    pub(crate) shape: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct FigureRecord {
    pub(crate) positions: u64,
    pub(crate) epoch_start: u64,
    pub(crate) rest: u64,
    pub(crate) traits: u64,
    pub(crate) triangles: u64,
    pub(crate) pieces: u64,
    pub(crate) body_from: u64,
    pub(crate) body_to: u64,
    pub(crate) body_triangles: u64,
    pub(crate) incident_starts: u64,
    pub(crate) incident: u64,
    pub(crate) epoch_motion: u64,
    pub(crate) counts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct JobRecord {
    pub(crate) positions: u64,
    pub(crate) velocities: u64,
    pub(crate) drift: u64,
    pub(crate) starts: u64,
    pub(crate) preds: u64,
    pub(crate) epoch_start: u64,
    pub(crate) targets_from: u64,
    pub(crate) targets_to: u64,
    pub(crate) rest: u64,
    pub(crate) traits: u64,
    pub(crate) edges: u64,
    pub(crate) bends: u64,
    pub(crate) shapes: u64,
    pub(crate) tether_starts: u64,
    pub(crate) tethers: u64,
    pub(crate) plan: u64,
    pub(crate) triangles: u64,
    pub(crate) incident_starts: u64,
    pub(crate) incident: u64,
    pub(crate) normals: u64,
    pub(crate) pieces: u64,
    pub(crate) stats: u64,
    pub(crate) slots: u64,
    pub(crate) gather_starts: u64,
    pub(crate) wrap_dests: u64,
    pub(crate) geometry: u64,
    pub(crate) contacts: u64,
    pub(crate) wraps: u64,
    pub(crate) selfs: u64,
    pub(crate) figures: u64,
    pub(crate) body_from: u64,
    pub(crate) body_to: u64,
    pub(crate) body_triangles: u64,
    pub(crate) colliders_from: u64,
    pub(crate) colliders_to: u64,
    pub(crate) planes_from: u64,
    pub(crate) planes_to: u64,
    pub(crate) cells: u64,
    pub(crate) items: u64,
    pub(crate) spheres: u64,
    pub(crate) large: u64,
    pub(crate) nearest: u64,
    pub(crate) epoch_motion: u64,
    pub(crate) grabbed: u64,
    pub(crate) grabs_from: u64,
    pub(crate) grabs_to: u64,
    pub(crate) range: [u32; 4],
    pub(crate) colours: [u32; 4],
    pub(crate) items_range: [u32; 4],
    pub(crate) shapes_figure: [u32; 4],
    pub(crate) steps: [u32; 4],
    pub(crate) flags: [u32; 4],
    pub(crate) step: [f32; 4],
    pub(crate) gravity: [f32; 4],
    pub(crate) wind: [f32; 4],
    pub(crate) rest_terms: [f32; 4],
}

#[derive(Default)]
pub(crate) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub(crate) fn section<T: Pod>(&mut self, items: &[T]) -> u64 {
        let at = self.bytes.len().next_multiple_of(16);
        self.bytes.resize(at, 0);
        self.bytes.extend_from_slice(bytemuck::cast_slice(items));
        at as u64
    }

    pub(crate) fn finish(mut self) -> Vec<u8> {
        let size = self.bytes.len().next_multiple_of(16).max(16);
        self.bytes.resize(size, 0);
        self.bytes
    }
}

pub(crate) fn octahedral(direction: glam::Vec3) -> [f32; 2] {
    let sum = direction.x.abs() + direction.y.abs() + direction.z.abs();
    if sum <= 1.0e-20 {
        return [0.0, 0.0];
    }
    let p = direction / sum;
    if p.z >= 0.0 {
        return [p.x, p.y];
    }
    let sign = |value: f32| if value >= 0.0 { 1.0 } else { -1.0 };
    [(1.0 - p.y.abs()) * sign(p.x), (1.0 - p.x.abs()) * sign(p.y)]
}
