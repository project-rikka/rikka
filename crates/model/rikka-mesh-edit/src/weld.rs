use smallvec::SmallVec;

use crate::core::DeleteContext;
use crate::geometry_set::Geometry;
use crate::ids::{EdgeId, Element, FaceId, LoopId, VertId};
use crate::mesh::EditMesh;
use crate::sets::{IdMap, IdSet, LocalSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WeldOptions {
    pub use_centroid: bool,
    pub average_vert_data: bool,
}

impl EditMesh {
    pub fn weld_verts(&mut self, targets: &IdMap<VertId, VertId>, options: &WeldOptions) {
        weld_verts(self, targets, options);
    }

    #[must_use]
    pub fn contains_element(&self, element: Element) -> bool {
        match element {
            Element::Vert(vert) => self.contains_vert(vert),
            Element::Edge(edge) => self.contains_edge(edge),
            Element::Face(face) => self.contains_face(face),
        }
    }
}

fn weld_verts(mesh: &mut EditMesh, targets: &IdMap<VertId, VertId>, options: &WeldOptions) {
    let average = options.average_vert_data || options.use_centroid;
    let mut history = History::new(mesh);
    let track = !history.is_empty();
    let mut map = ElementMap::default();
    let mut doomed = Geometry::new();
    let mut clusters: IdMap<VertId, Vec<VertId>> = IdMap::new();
    let mut survivors: Vec<VertId> = Vec::new();

    let verts: Vec<VertId> = mesh.verts().collect();
    for &vert in &verts {
        let Some(target) = targets.copied(vert) else {
            continue;
        };
        doomed.verts.insert(vert);
        mesh.merge_vert_flags(vert, target);
        if track {
            map.verts.insert(vert, target);
        }
        if average {
            if !clusters.contains(target) {
                survivors.push(target);
            }
            clusters.entry_or_insert_with(target, Vec::new).push(vert);
        }
    }
    if doomed.verts.is_empty() {
        return;
    }

    if options.use_centroid {
        for &target in &survivors {
            let Some(cluster) = clusters.get(target) else {
                continue;
            };
            let mut centroid = mesh.position(target);
            for &duplicate in cluster {
                centroid += mesh.position(duplicate);
            }
            centroid *= 1.0 / (cluster.len() + 1) as f32;
            mesh.set_position(target, centroid);
        }
    }

    if average {
        for &target in &survivors {
            let Some(cluster) = clusters.get(target) else {
                continue;
            };
            let mut sources: Vec<VertId> = Vec::with_capacity(cluster.len() + 1);
            sources.push(target);
            sources.extend_from_slice(cluster);
            mesh.average_vert_data(&sources, target);
        }
    }

    let faces: Vec<FaceId> = mesh.faces().collect();
    for &face in &faces {
        split_self_merged(mesh, face, targets);
    }

    let mapped = |vert: VertId| match doomed.verts.contains(vert) {
        true => targets.copied(vert).unwrap_or(vert),
        false => vert,
    };

    let mut collapsed: IdSet<EdgeId> = IdSet::new();
    let edges: Vec<EdgeId> = mesh.edges().collect();
    for &edge in &edges {
        let [a, b] = mesh.edge_verts(edge);
        if !doomed.verts.contains(a) && !doomed.verts.contains(b) {
            continue;
        }
        let (a, b) = (mapped(a), mapped(b));
        if a == b {
            collapsed.insert(edge);
        } else {
            let replacement = match mesh.edge_between(a, b) {
                Some(existing) => Some(existing),
                None => mesh.add_edge_like(a, b, edge).ok(),
            };
            if let Some(replacement) = replacement {
                mesh.merge_edge_flags(replacement, edge);
                if track {
                    map.edges.insert(edge, replacement);
                }
            }
        }
        doomed.edges.insert(edge);
    }

    let faces: Vec<FaceId> = mesh.faces().collect();
    for &face in &faces {
        let mut touches_doomed = false;
        let mut collapsing = 0;
        for lp in mesh.face_loops(face) {
            if doomed.verts.contains(mesh.loop_vert(lp)) {
                touches_doomed = true;
            }
            if collapsed.contains(mesh.loop_edge(lp)) {
                collapsing += 1;
            }
        }
        if !touches_doomed {
            continue;
        }
        doomed.faces.insert(face);
        if mesh.face_len(face) - collapsing < 3 {
            continue;
        }
        match rebuild_face(mesh, face, &doomed.verts, targets) {
            Rebuilt::Created(created) => {
                if replace_in_place(mesh, face, created) {
                    doomed.faces.remove(face);
                }
            }
            Rebuilt::Existing(existing) => {
                mesh.merge_face_flags(existing, face);
                if track {
                    map.faces.insert(face, existing);
                }
                history.remap_active_face(face, existing);
            }
            Rebuilt::Failed => {}
        }
    }

    if track {
        history.merge(&map);
    }
    mesh.delete(DeleteContext::OnlyTagged, &doomed);
    history.restore(mesh);
}

fn split_self_merged(mesh: &mut EditMesh, face: FaceId, targets: &IdMap<VertId, VertId>) {
    let mut pending: SmallVec<[FaceId; 8]> = SmallVec::new();
    pending.push(face);
    while let Some(face) = pending.pop() {
        let mut split: Option<(LoopId, LoopId)> = None;
        for lp in mesh.face_loops(face) {
            let Some(target) = targets.copied(mesh.loop_vert(lp)) else {
                continue;
            };
            if let Some(at_target) = mesh.face_vert_share_loop(face, target)
                && at_target != lp
                && !mesh.is_loop_adjacent(at_target, lp)
            {
                split = Some((lp, at_target));
                break;
            }
        }
        let Some((double, at_target)) = split else {
            continue;
        };
        if let Ok(made) = mesh.split_face(face, double, at_target, None, false) {
            pending.push(made.face);
            pending.push(face);
        }
    }
}

enum Rebuilt {
    Created(FaceId),
    Existing(FaceId),
    Failed,
}

fn rebuild_face(
    mesh: &mut EditMesh,
    face: FaceId,
    doomed: &IdSet<VertId>,
    targets: &IdMap<VertId, VertId>,
) -> Rebuilt {
    let mapped = |vert: VertId| match doomed.contains(vert) {
        true => (targets.copied(vert).unwrap_or(vert), true),
        false => (vert, false),
    };
    let loops = mesh.face_loops_vec(face);
    let mut verts: SmallVec<[VertId; 8]> = SmallVec::new();
    let mut sources: SmallVec<[LoopId; 8]> = SmallVec::new();
    let mut seen: LocalSet<VertId> = LocalSet::new();
    let count = loops.len();
    for index in 0..count {
        let current = loops[index];
        let next = loops[(index + 1) % count];
        let (vert, doomed_vert) = mapped(mesh.loop_vert(current));
        let (next_vert, doomed_next) = mapped(mesh.loop_vert(next));
        let keeps_edge = match (doomed_vert || doomed_next, vert == next_vert) {
            (false, _) => true,
            (true, true) => false,
            (true, false) => match mesh.edge_between(vert, next_vert) {
                Some(_) => true,
                None => return Rebuilt::Failed,
            },
        };
        if !keeps_edge {
            continue;
        }
        if !seen.insert(vert) {
            return Rebuilt::Failed;
        }
        verts.push(vert);
        sources.push(current);
    }
    if verts.len() < 3 {
        return Rebuilt::Failed;
    }
    if let Some(existing) = mesh.face_exists(&verts) {
        return Rebuilt::Existing(existing);
    }
    let Ok(created) = mesh
        .new_face(&verts)
        .existing_edges()
        .example(face)
        .create()
    else {
        return Rebuilt::Failed;
    };
    let created_loops = mesh.face_loops_vec(created);
    for (&source, &target) in sources.iter().zip(&created_loops) {
        mesh.copy_loop_data(source, target);
    }
    Rebuilt::Created(created)
}

fn replace_in_place(mesh: &mut EditMesh, face: FaceId, created: FaceId) -> bool {
    let flags = mesh.face_flags(face);
    if mesh.kill_face(face).is_err() {
        return false;
    }
    let Ok(copy) = mesh.copy_face(created, false, false) else {
        return false;
    };
    mesh.set_face_flags(copy, flags);
    mesh.kill_face(created).is_ok() && copy == face
}

#[derive(Clone, Debug, Default)]
struct ElementMap {
    verts: IdMap<VertId, VertId>,
    edges: IdMap<EdgeId, EdgeId>,
    faces: IdMap<FaceId, FaceId>,
}

impl ElementMap {
    fn next(&self, element: Element) -> Option<Element> {
        match element {
            Element::Vert(vert) => self.verts.copied(vert).map(Element::Vert),
            Element::Edge(edge) => self.edges.copied(edge).map(Element::Edge),
            Element::Face(face) => self.faces.copied(face).map(Element::Face),
        }
    }
}

#[derive(Clone, Debug)]
struct History {
    elements: Vec<Element>,
    active_face: Option<FaceId>,
}

impl History {
    fn new(mesh: &EditMesh) -> History {
        History {
            elements: mesh.select_history().to_vec(),
            active_face: mesh.active_face(),
        }
    }

    fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    fn merge(&mut self, map: &ElementMap) {
        let mut overlap: Vec<Element> = self.elements.clone();
        for element in &mut self.elements {
            let start = *element;
            let mut target = start;
            while let Some(next) = map.next(target) {
                target = next;
                if target == start {
                    break;
                }
            }
            *element = target;
        }
        let elements = std::mem::take(&mut self.elements);
        for element in elements {
            if let Some(position) = overlap.iter().position(|&kept| kept == element) {
                overlap.swap_remove(position);
                self.elements.push(element);
            }
        }
    }

    fn remap_active_face(&mut self, from: FaceId, to: FaceId) {
        if self.active_face == Some(from) {
            self.active_face = Some(to);
        }
    }

    fn restore(&self, mesh: &mut EditMesh) {
        mesh.history_clear();
        for &element in &self.elements {
            if mesh.contains_element(element) {
                mesh.history_store(element);
            }
        }
        mesh.set_active_face(self.active_face);
    }
}
