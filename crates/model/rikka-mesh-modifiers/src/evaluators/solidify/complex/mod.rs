mod adjacency;
mod groups;
mod output;
mod placement;

use glam::Vec3;
use rikka_polymesh::Polymesh;

use super::super::vertex_group::Group;
use super::MaterialOffsets;
use crate::kinds::solidify::Solidify;
use crate::normals;
use crate::note::SkipReason;

pub(super) const EMPTY: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GroupId {
    vert: u32,
    index: u32,
}

#[derive(Clone, Debug)]
pub(super) struct NewEdge {
    old_edge: u32,
    faces: [Option<u32>; 2],
    groups: [Option<GroupId>; 2],
    angle: f32,
    new_edge: u32,
}

#[derive(Clone, Debug)]
pub(super) struct EdgeGroup {
    edges: Vec<u32>,
    open_face_edge: u32,
    is_orig_closed: bool,
    is_even_split: bool,
    split: u32,
    is_singularity: bool,
    topo_group: u32,
    co: Vec3,
    new_vert: u32,
}

impl EdgeGroup {
    fn new(edges: Vec<u32>, topo_group: u32) -> EdgeGroup {
        EdgeGroup {
            edges,
            open_face_edge: EMPTY,
            is_orig_closed: true,
            is_even_split: false,
            split: 0,
            is_singularity: false,
            topo_group,
            co: Vec3::ZERO,
            new_vert: EMPTY,
        }
    }
}

pub(super) struct Settings {
    shell: bool,
    rim: bool,
    flip: bool,
    ofs_front_clamped: f32,
    ofs_back_clamped: f32,
    group: Group,
    invert: bool,
    flat_faces: bool,
    materials: MaterialOffsets,
}

pub(super) struct Topology {
    vm: Vec<u32>,
    coords: Vec<Vec3>,
    face_normals: Vec<Vec3>,
    null_faces: Vec<bool>,
    edge_lengths: Vec<f32>,
    adj_len: Vec<u32>,
    edges: Vec<NewEdge>,
    lists: Vec<Vec<u32>>,
    edge_list: Vec<Option<u32>>,
    side_links: Vec<Vec<Option<u32>>>,
    vert_edges: Vec<Vec<u32>>,
    groups: Vec<Vec<EdgeGroup>>,
    has_singularities: bool,
}

impl Topology {
    fn group(&self, id: Option<GroupId>) -> Result<&EdgeGroup, SkipReason> {
        let id = id.ok_or(SkipReason::InvalidMesh)?;
        self.groups
            .get(id.vert as usize)
            .and_then(|groups| groups.get(id.index as usize))
            .ok_or(SkipReason::InvalidMesh)
    }

    fn list_owner(&self, edge: usize) -> Option<&[u32]> {
        let list = &self.lists[self.edge_list[edge]? as usize];
        let first = *list.first()?;
        (self.edges[first as usize].old_edge as usize == edge).then_some(list.as_slice())
    }
}

fn other_side(side: u32) -> u32 {
    side ^ 1
}

fn clamp_nonzero(value: f32, epsilon: f32) -> f32 {
    match value < 0.0 {
        true => value.min(-epsilon),
        false => value.max(epsilon),
    }
}

pub(super) fn normalize_length(vector: Vec3) -> (Vec3, f32) {
    let squared = vector.dot(vector);
    match squared > 1.0e-35 {
        true => {
            let length = squared.sqrt();
            (vector * (1.0 / length), length)
        }
        false => (Vec3::ZERO, 0.0),
    }
}

pub(crate) fn evaluate(
    solidify: &Solidify,
    mesh: &mut Polymesh,
    materials: MaterialOffsets,
) -> Result<(), SkipReason> {
    if mesh.face_count() == 0 && mesh.point_count() != 0 {
        return Ok(());
    }
    let ofs_front = (solidify.offset + 1.0) * 0.5 * solidify.thickness;
    let ofs_back = ofs_front - solidify.thickness * solidify.offset;
    let group = Group::find(mesh, &solidify.vertex_group);
    let weighted = matches!(group, Group::Weighted(_));
    let settings = Settings {
        shell: !(solidify.use_rim && solidify.use_rim_only),
        rim: solidify.use_rim,
        flip: solidify.use_flip_normals == (solidify.thickness > 0.0),
        ofs_front_clamped: clamp_nonzero(ofs_front, 1.0e-5),
        ofs_back_clamped: clamp_nonzero(ofs_back, 1.0e-5),
        group,
        invert: solidify.invert_vertex_group,
        flat_faces: weighted && solidify.use_flat_faces,
        materials,
    };
    let mut working = mesh.clone();
    let carried = normals::encode_custom(&mut working);
    let mut topology = adjacency::build(solidify, &working, &settings);
    groups::build(&mut topology, &working, &settings);
    placement::place(solidify, &mut topology, &working, &settings);
    let mut result = output::assemble(solidify, &mut topology, &working, &settings)?;
    if carried {
        normals::decode_custom(&mut result);
    }
    *mesh = result;
    Ok(())
}
