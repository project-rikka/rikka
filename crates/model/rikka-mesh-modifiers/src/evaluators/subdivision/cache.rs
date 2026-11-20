use glam::Vec3;
use rikka_polymesh::{Attributes, Polymesh, ShapeKey, ShapeKeys, Values, VertexGroups};

use super::Chain;
use crate::kinds::subdivision::Subdivision;

pub(crate) struct Cached {
    settings: Subdivision,
    levels: u32,
    input: Snapshot,
    output: Polymesh,
    pub(super) chain: Chain,
}

pub(super) struct Snapshot {
    points: usize,
    mesh: Polymesh,
}

pub(super) fn snapshot(mesh: &Polymesh) -> Snapshot {
    let keys = mesh
        .shape_keys
        .keys
        .iter()
        .map(|key| ShapeKey {
            name: key.name.clone(),
            positions: Vec::new(),
            value: key.value,
            min: key.min,
            max: key.max,
            relative_to: key.relative_to,
            vertex_group: key.vertex_group.clone(),
            muted: key.muted,
        })
        .collect();
    Snapshot {
        points: mesh.point_count(),
        mesh: Polymesh {
            positions: Vec::new(),
            edges: mesh.edges.clone(),
            face_offsets: mesh.face_offsets.clone(),
            corner_verts: mesh.corner_verts.clone(),
            corner_edges: mesh.corner_edges.clone(),
            attributes: mesh.attributes.clone(),
            uv_maps: mesh.uv_maps.clone(),
            color_attributes: mesh.color_attributes.clone(),
            vertex_groups: mesh.vertex_groups.clone(),
            shape_keys: ShapeKeys {
                keys,
                active: mesh.shape_keys.active,
            },
        },
    }
}

impl Cached {
    pub(super) fn new(
        settings: Subdivision,
        levels: u32,
        input: Snapshot,
        output: &Polymesh,
        chain: Chain,
    ) -> Cached {
        Cached {
            settings,
            levels,
            input,
            output: output.clone(),
            chain,
        }
    }

    pub(super) fn matches(&self, settings: &Subdivision, levels: u32, mesh: &Polymesh) -> bool {
        self.settings == *settings
            && self.levels == levels
            && mesh.point_count() == self.input.points
            && same_except_positions(&self.input.mesh, mesh)
    }

    pub(super) fn reuse(&self, mesh: &mut Polymesh) {
        let positions = self.chain.apply(&mesh.positions);
        let keys: Vec<Vec<Vec3>> = mesh
            .shape_keys
            .keys
            .iter()
            .map(|key| self.chain.apply(&key.positions))
            .collect();
        *mesh = self.output.clone();
        mesh.positions = positions;
        for (key, positions) in mesh.shape_keys.keys.iter_mut().zip(keys) {
            key.positions = positions;
        }
    }
}

fn same_floats(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn same_arrays<T: Copy, const N: usize>(a: &[T], b: &[T], bits: impl Fn(T) -> [f32; N]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(&x, &y)| same_floats(&bits(x), &bits(y)))
}

fn same_values(a: &Values, b: &Values) -> bool {
    match (a, b) {
        (Values::Bool(a), Values::Bool(b)) => a == b,
        (Values::Int(a), Values::Int(b)) => a == b,
        (Values::Float(a), Values::Float(b)) => same_floats(a, b),
        (Values::Vec2(a), Values::Vec2(b)) => same_arrays(a, b, |value| value.to_array()),
        (Values::Vec3(a), Values::Vec3(b)) => same_arrays(a, b, |value| value.to_array()),
        (Values::Color(a), Values::Color(b)) => same_arrays(a, b, |value| value.to_array()),
        _ => false,
    }
}

fn same_attributes(a: &Attributes, b: &Attributes) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(a, b)| {
            a.name == b.name && a.domain == b.domain && same_values(&a.values, &b.values)
        })
}

fn same_groups(a: &VertexGroups, b: &VertexGroups) -> bool {
    a.groups == b.groups
        && a.active == b.active
        && a.weights.len() == b.weights.len()
        && a.weights.iter().zip(&b.weights).all(|(a, b)| {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(a, b)| a.group == b.group && a.weight.to_bits() == b.weight.to_bits())
        })
}

fn same_keys(a: &ShapeKeys, b: &ShapeKeys, points: usize) -> bool {
    a.active == b.active
        && a.keys.len() == b.keys.len()
        && a.keys.iter().zip(&b.keys).all(|(a, b)| {
            b.positions.len() == points
                && a.name == b.name
                && same_floats(&[a.value, a.min, a.max], &[b.value, b.min, b.max])
                && a.relative_to == b.relative_to
                && a.vertex_group == b.vertex_group
                && a.muted == b.muted
        })
}

fn same_except_positions(input: &Polymesh, mesh: &Polymesh) -> bool {
    input.edges == mesh.edges
        && input.face_offsets == mesh.face_offsets
        && input.corner_verts == mesh.corner_verts
        && input.corner_edges == mesh.corner_edges
        && input.uv_maps == mesh.uv_maps
        && input.color_attributes == mesh.color_attributes
        && same_groups(&input.vertex_groups, &mesh.vertex_groups)
        && same_keys(&input.shape_keys, &mesh.shape_keys, mesh.point_count())
        && same_attributes(&input.attributes, &mesh.attributes)
}
