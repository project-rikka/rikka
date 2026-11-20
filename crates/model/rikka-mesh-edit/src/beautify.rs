use rustc_hash::FxHashSet;

use crate::error::Result;
use crate::geometry_set::Geometry;
use crate::heap::{Heap, HeapNode};
use crate::ids::{EdgeId, VertId};
use crate::math::{rotate_beauty_angle, rotate_beauty_area};
use crate::mesh::EditMesh;
use crate::mods::{RotateChecks, Rotation};
use crate::sets::{IdMap, IdSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BeautyMethod {
    #[default]
    Area,
    Angle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct RotationState {
    verts: [u32; 2],
    opposite: [u32; 2],
}

fn ordered(a: VertId, b: VertId) -> [u32; 2] {
    let (a, b) = (a.raw(), b.raw());
    [a.min(b), a.max(b)]
}

fn edge_quad(mesh: &EditMesh, edge: EdgeId) -> Option<[VertId; 4]> {
    let lp = mesh.edge_first_loop(edge)?;
    let radial = mesh.loop_radial_next(lp);
    if radial == lp {
        return None;
    }
    Some([
        mesh.loop_vert(mesh.loop_prev(lp)),
        mesh.loop_vert(lp),
        mesh.loop_vert(mesh.loop_prev(radial)),
        mesh.loop_next_vert(lp),
    ])
}

fn current_state(mesh: &EditMesh, edge: EdgeId) -> Option<RotationState> {
    let quad = edge_quad(mesh, edge)?;
    let [a, b] = mesh.edge_verts(edge);
    Some(RotationState {
        verts: ordered(a, b),
        opposite: ordered(quad[0], quad[2]),
    })
}

fn alternate_state(mesh: &EditMesh, edge: EdgeId) -> Option<RotationState> {
    current_state(mesh, edge).map(|state| RotationState {
        verts: state.opposite,
        opposite: state.verts,
    })
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BeautifyFill<'a> {
    pub method: BeautyMethod,
    pub lock_degenerate: bool,
    pub restrict: Option<&'a IdSet<VertId>>,
}

fn rotate_beauty(mesh: &EditMesh, quad: [VertId; 4], settings: &BeautifyFill) -> f32 {
    let [v1, v2, v3, v4] = quad;
    if let Some(restrict) = settings.restrict
        && restrict.contains(v1) == restrict.contains(v3)
    {
        return f32::MAX;
    }
    if v1 == v3 {
        return f32::MAX;
    }
    let (p1, p2, p3, p4) = (
        mesh.position(v1),
        mesh.position(v2),
        mesh.position(v3),
        mesh.position(v4),
    );
    match settings.method {
        BeautyMethod::Area => rotate_beauty_area(p1, p2, p3, p4, settings.lock_degenerate),
        BeautyMethod::Angle => rotate_beauty_angle(p1, p2, p3, p4),
    }
}

fn edge_cost(mesh: &EditMesh, edge: EdgeId, settings: &BeautifyFill) -> f32 {
    match edge_quad(mesh, edge) {
        Some(quad) => rotate_beauty(mesh, quad, settings),
        None => f32::MAX,
    }
}

struct Fill<'a, 'b> {
    edges: Vec<EdgeId>,
    index: IdMap<EdgeId, usize>,
    nodes: Vec<Option<HeapNode>>,
    states: Vec<FxHashSet<RotationState>>,
    heap: Heap<EdgeId>,
    settings: &'a BeautifyFill<'b>,
}

impl Fill<'_, '_> {
    fn update_single(&mut self, mesh: &EditMesh, edge: EdgeId) {
        let Some(&position) = self.index.get(edge) else {
            return;
        };
        if self.edges[position] != edge {
            return;
        }
        if let Some(node) = self.nodes[position].take() {
            self.heap.remove(node);
        }
        let Some(alternate) = alternate_state(mesh, edge) else {
            return;
        };
        if self.states[position].contains(&alternate) {
            return;
        }
        let cost = edge_cost(mesh, edge, self.settings);
        if cost < 0.0 {
            self.nodes[position] = Some(self.heap.insert(cost, edge));
        }
    }

    fn update_around(&mut self, mesh: &EditMesh, edge: EdgeId) {
        let Some(lp) = mesh.edge_first_loop(edge) else {
            return;
        };
        let radial = mesh.loop_radial_next(lp);
        let neighbours = [
            mesh.loop_edge(mesh.loop_next(lp)),
            mesh.loop_edge(mesh.loop_prev(lp)),
            mesh.loop_edge(mesh.loop_next(radial)),
            mesh.loop_edge(mesh.loop_prev(radial)),
        ];
        for neighbour in neighbours {
            self.update_single(mesh, neighbour);
        }
    }
}

impl EditMesh {
    pub fn beautify_fill(&mut self, edges: &[EdgeId], settings: &BeautifyFill) -> Result<Geometry> {
        beautify_fill(self, edges.to_vec(), settings)
    }
}

fn beautify_fill(
    mesh: &mut EditMesh,
    edges: Vec<EdgeId>,
    settings: &BeautifyFill,
) -> Result<Geometry> {
    let mut fill = Fill {
        nodes: vec![None; edges.len()],
        states: vec![FxHashSet::default(); edges.len()],
        index: IdMap::with_capacity(edges.len()),
        heap: Heap::with_capacity(edges.len()),
        edges,
        settings,
    };
    for position in 0..fill.edges.len() {
        let edge = fill.edges[position];
        let cost = edge_cost(mesh, edge, settings);
        if cost < 0.0 {
            fill.nodes[position] = Some(fill.heap.insert(cost, edge));
        }
        fill.index.insert(edge, position);
    }
    let mut output = Geometry::new();
    while let Some(edge) = fill.heap.pop_min() {
        let Some(&position) = fill.index.get(edge) else {
            continue;
        };
        fill.nodes[position] = None;
        let Ok(rotated) = mesh.rotate_edge(edge, Rotation::Clockwise, RotateChecks::EXISTS) else {
            continue;
        };
        if let Some(state) = current_state(mesh, rotated) {
            fill.states[position].insert(state);
        }
        fill.edges[position] = rotated;
        fill.index.insert(rotated, position);
        fill.update_around(mesh, rotated);
        output.edges.insert(rotated);
        for face in mesh.edge_faces(rotated) {
            output.faces.insert(face);
        }
    }
    output.edges.retain(|edge| mesh.contains_edge(edge));
    output.faces.retain(|face| mesh.contains_face(face));
    Ok(output)
}
