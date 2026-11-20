use glam::{Vec2, Vec3};

use crate::error::Result;
use crate::ids::{EdgeId, Element, FaceId, LoopId, VertId};
use crate::math::{angle_signed_on_axis, axis_dominant, normalize_2d, project_2d};
use crate::mesh::{EditMesh, Flags};
use crate::sets::IdSet;

struct EdgeNet<'a> {
    mesh: &'a mut EditMesh,
    face: FaceId,
    normal: Vec3,
    matrix: glam::Mat3,
    net_edges: IdSet<EdgeId>,
    net_faces: IdSet<FaceId>,
}

impl EdgeNet<'_> {
    fn radial_count(&self, edge: EdgeId) -> usize {
        self.mesh
            .edge_loops(edge)
            .filter(|&lp| self.net_faces.contains(self.mesh.loop_face(lp)))
            .count()
    }

    fn radial_first(&self, edge: EdgeId) -> Option<LoopId> {
        self.mesh
            .edge_loops(edge)
            .find(|&lp| self.net_faces.contains(self.mesh.loop_face(lp)))
    }

    fn direction_2d(&self, a: VertId, b: VertId) -> Vec2 {
        normalize_2d(project_2d(
            &self.matrix,
            self.mesh.position(a) - self.mesh.position(b),
        ))
    }

    fn pair_counts(&self, vert: VertId) -> (Vec<EdgeId>, Vec<EdgeId>) {
        let mut boundary = Vec::new();
        let mut wire = Vec::new();
        for edge in self.mesh.vert_edges(vert).collect::<Vec<_>>() {
            if self.net_edges.contains(edge) {
                match self.radial_count(edge) {
                    1 => boundary.push(edge),
                    0 => wire.push(edge),
                    _ => {}
                }
            }
        }
        (boundary, wire)
    }

    fn pair_exists(&self, vert: VertId) -> bool {
        let (boundary, wire) = self.pair_counts(vert);
        !boundary.is_empty() && (!wire.is_empty() || boundary.len() >= 2)
    }

    fn find_pair(&self, vert: VertId) -> Option<[EdgeId; 2]> {
        let (mut boundary, mut wire) = self.pair_counts(vert);
        let first = boundary.pop()?;
        let mut search: Vec<EdgeId> = Vec::new();
        let second = match wire.is_empty() {
            true => {
                let second = boundary.pop()?;
                if !boundary.is_empty() {
                    search = boundary;
                }
                second
            }
            false => {
                let second = wire.pop()?;
                if !wire.is_empty() {
                    search = wire;
                }
                second
            }
        };
        let mut pair = [first, second];
        if !search.is_empty() {
            let previous = self.mesh.edge_other_vert(pair[0], vert)?;
            let next = self.mesh.edge_other_vert(pair[1], vert)?;
            let dir_prev = self.direction_2d(previous, vert);
            let dir_next = self.direction_2d(next, vert);
            let mut best = dir_next.dot(dir_prev);
            while let Some(edge) = search.pop() {
                let Some(other) = self.mesh.edge_other_vert(edge, vert) else {
                    continue;
                };
                let test = dir_prev.dot(self.direction_2d(other, vert));
                if test > best {
                    best = test;
                    pair[1] = edge;
                }
            }
        }
        let walk = self.radial_first(pair[0])?;
        let mut swap = false;
        if self.mesh.loop_face(walk) == self.face {
            swap = !swap;
        }
        if self.mesh.loop_vert(walk) != vert {
            swap = !swap;
        }
        if swap {
            pair.swap(0, 1);
        }
        Some(pair)
    }

    fn walk(&mut self, start: VertId, pair: [EdgeId; 2]) -> bool {
        let Some(first) = self.mesh.edge_other_vert(pair[0], start) else {
            return false;
        };
        self.mesh.set_disk_start(first, pair[0]);
        let Some(destination) = self.mesh.edge_other_vert(pair[1], start) else {
            return false;
        };
        let mut visited: IdSet<VertId> = IdSet::new();
        let mut stack: Vec<VertId> = vec![first];
        let mut stack_next: Vec<VertId> = Vec::new();
        while let Some(popped) = stack.pop() {
            let mut vert = popped;
            loop {
                if vert == destination {
                    return true;
                }
                let Some(previous_edge) = self.mesh.vert_first_edge(vert) else {
                    break;
                };
                let mut order: Vec<(f32, VertId)> = Vec::new();
                let mut edge = self.mesh.disk_next(previous_edge, vert);
                if edge != previous_edge {
                    loop {
                        if self.net_edges.contains(edge) && self.radial_count(edge) < 2 {
                            let next = self.mesh.edge_other_vert(edge, vert).unwrap_or(vert);
                            if !visited.contains(next) {
                                order.push((0.0, next));
                                self.mesh.set_disk_start(next, edge);
                            }
                        }
                        edge = self.mesh.disk_next(edge, vert);
                        if edge == previous_edge {
                            break;
                        }
                    }
                }
                if order.len() == 1 {
                    vert = order[0].1;
                    continue;
                }
                if order.len() > 1 {
                    let previous = self
                        .mesh
                        .edge_other_vert(previous_edge, vert)
                        .unwrap_or(vert);
                    let (p, v) = (self.mesh.position(previous), self.mesh.position(vert));
                    for entry in &mut order {
                        entry.0 =
                            angle_signed_on_axis(p, v, self.mesh.position(entry.1), self.normal);
                    }
                    order.sort_by(|a, b| b.0.total_cmp(&a.0));
                    visited.insert(vert);
                }
                while let Some((_, next)) = order.pop() {
                    stack_next.push(next);
                }
                if !stack_next.is_empty() {
                    std::mem::swap(&mut stack, &mut stack_next);
                }
                break;
            }
        }
        false
    }

    fn find_loop(&mut self, start: VertId) -> Option<(Vec<VertId>, bool)> {
        let pair = self.find_pair(start)?;
        if !self.walk(start, pair) {
            return None;
        }
        let mut check_exists = true;
        let mut verts = vec![start];
        if self.mesh.is_edge_wire(pair[1]) {
            check_exists = false;
        }
        let mut vert = self.mesh.edge_other_vert(pair[1], start)?;
        let limit = self.mesh.edge_capacity() + 2;
        loop {
            let edge = self.mesh.vert_first_edge(vert)?;
            verts.push(vert);
            if check_exists && self.mesh.is_edge_wire(edge) {
                check_exists = false;
            }
            vert = self.mesh.edge_other_vert(edge, vert)?;
            if vert == start || verts.len() > limit {
                break;
            }
        }
        if verts.len() < 3 {
            return None;
        }
        Some((verts, check_exists))
    }
}

impl EditMesh {
    pub fn split_face_edgenet(&mut self, face: FaceId, edge_net: &[EdgeId]) -> Result<Vec<FaceId>> {
        split_face_edgenet(self, face, edge_net)
    }

    pub(crate) fn set_disk_start(&mut self, vert: VertId, edge: EdgeId) {
        self.verts.records[vert.index()].edge = edge.0;
    }
}

fn split_face_edgenet(
    mesh: &mut EditMesh,
    face: FaceId,
    edge_net: &[EdgeId],
) -> Result<Vec<FaceId>> {
    if edge_net.is_empty() || !mesh.contains_face(face) {
        return Ok(Vec::new());
    }
    let normal = mesh.face_normal(face);
    let face_flags = mesh.face_flags(face);
    let mut net_edges: IdSet<EdgeId> = edge_net.iter().copied().collect();
    let face_loops = mesh.face_loops_vec(face);
    for &lp in &face_loops {
        net_edges.insert(mesh.loop_edge(lp));
    }
    let mut net_faces = IdSet::new();
    net_faces.insert(face);
    let first_vert = mesh.loop_vert(face_loops[0]);
    let mut net = EdgeNet {
        matrix: axis_dominant(normal),
        mesh,
        face,
        normal,
        net_edges,
        net_faces,
    };
    let mut queue: Vec<VertId> = vec![first_vert];
    let mut in_queue: IdSet<VertId> = IdSet::new();
    in_queue.insert(first_vert);
    let mut created: Vec<FaceId> = Vec::new();
    while let Some(vert) = queue.pop() {
        in_queue.remove(vert);
        let Some((verts, check_exists)) = net.find_loop(vert) else {
            continue;
        };
        if check_exists && net.mesh.face_exists(&verts).is_some() {
            continue;
        }
        let Ok(new_face) = net
            .mesh
            .new_face(&verts)
            .existing_edges()
            .example(face)
            .create()
        else {
            continue;
        };
        net.mesh.set_face_normal(new_face, normal);
        net.mesh
            .set_face_flag(new_face, Flags::SELECT, face_flags.contains(Flags::SELECT));
        net.mesh
            .set_face_flag(new_face, Flags::HIDDEN, face_flags.contains(Flags::HIDDEN));
        net.net_faces.insert(new_face);
        created.push(new_face);
        for lp in net.mesh.face_loops_vec(new_face) {
            let corner = net.mesh.loop_vert(lp);
            if !in_queue.contains(corner) && net.pair_exists(corner) {
                queue.push(corner);
                in_queue.insert(corner);
            }
        }
    }
    let net_faces = net.net_faces.clone();
    let mesh = &mut *net.mesh;
    if mesh.has_loop_math() && !created.is_empty() {
        let sample = mesh.sample_face(face);
        let mut visited: IdSet<VertId> = IdSet::new();
        for &lp in &face_loops {
            let vert = mesh.loop_vert(lp);
            let others: Vec<LoopId> = mesh
                .vert_loops(vert)
                .filter(|&other| {
                    mesh.loop_face(other) != face && net_faces.contains(mesh.loop_face(other))
                })
                .collect();
            for other in others {
                mesh.copy_loop_data(lp, other);
            }
            visited.insert(vert);
        }
        for &edge in edge_net {
            if !mesh.contains_edge(edge) {
                continue;
            }
            for vert in mesh.edge_verts(edge) {
                if !visited.insert(vert) {
                    continue;
                }
                let loops: Vec<LoopId> = mesh
                    .vert_loops(vert)
                    .filter(|&lp| net_faces.contains(mesh.loop_face(lp)))
                    .collect();
                let Some((&first, rest)) = loops.split_first() else {
                    continue;
                };
                mesh.interpolate_loop_from_sample(first, &sample, false);
                for &other in rest {
                    mesh.copy_loop_data(first, other);
                }
            }
        }
    }
    if created.is_empty() {
        return Ok(created);
    }
    let first_new = created[0];
    let verts = mesh.face_verts_vec(first_new);
    let first_loops = mesh.face_loops_vec(first_new);
    let flags = mesh.face_flags(face);
    let was_active = mesh.active_face() == Some(face);
    let history: Vec<Element> = mesh.select_history().to_vec();
    mesh.kill_face(face)?;
    let rebuilt = mesh
        .new_face(&verts)
        .existing_edges()
        .example(first_new)
        .create()?;
    for (source, target) in first_loops.iter().zip(mesh.face_loops_vec(rebuilt)) {
        mesh.copy_loop_data(*source, target);
    }
    mesh.set_face_normal(rebuilt, normal);
    mesh.set_face_flag(rebuilt, Flags::SELECT, flags.contains(Flags::SELECT));
    mesh.kill_face(first_new)?;
    if was_active {
        mesh.set_active_face(Some(rebuilt));
    }
    if history.contains(&Element::Face(face)) {
        mesh.history_clear();
        for element in history {
            let element = match element {
                Element::Face(old) if old == face => Element::Face(rebuilt),
                other => other,
            };
            mesh.history_store(element);
        }
    }
    created[0] = rebuilt;
    Ok(created)
}
