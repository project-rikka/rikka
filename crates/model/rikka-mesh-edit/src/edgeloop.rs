use std::collections::VecDeque;

use glam::Vec3;

use crate::error::Result;
use crate::ids::{EdgeId, VertId};
use crate::math::{newell_cross, normalize_with_length};
use crate::mesh::EditMesh;
use crate::sets::IdSet;

const EDGELOOP_EPS: f32 = 1.0e-10;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdgeLoop {
    pub verts: VecDeque<VertId>,
    pub closed: bool,
    pub center: Vec3,
    pub normal: Vec3,
}

fn other_tagged(
    mesh: &EditMesh,
    vert: VertId,
    previous: VertId,
    edge_tags: &IdSet<EdgeId>,
) -> (usize, Option<EdgeId>) {
    let mut count = 0;
    let mut next = None;
    for edge in mesh.vert_edges(vert) {
        if edge_tags.contains(edge)
            && let Some(other) = mesh.edge_other_vert(edge, vert)
            && other != previous
        {
            next = Some(edge);
            count += 1;
        }
    }
    (count, next)
}

fn build(
    mesh: &EditMesh,
    store: &mut EdgeLoop,
    mut previous: VertId,
    start: VertId,
    head: bool,
    vert_tags: &mut IdSet<VertId>,
    edge_tags: &mut IdSet<EdgeId>,
) -> bool {
    if !vert_tags.contains(start) {
        return true;
    }
    let first = start;
    let mut current = Some(start);
    while let Some(vert) = current {
        match head {
            true => store.verts.push_front(vert),
            false => store.verts.push_back(vert),
        }
        vert_tags.remove(vert);
        let (count, next_edge) = other_tagged(mesh, vert, previous, edge_tags);
        let next = match (count, next_edge) {
            (1, Some(edge)) => {
                let next = mesh.edge_other_vert(edge, vert);
                edge_tags.remove(edge);
                if next == Some(first) {
                    store.closed = true;
                    None
                } else {
                    next
                }
            }
            (0, _) => None,
            _ => return false,
        };
        previous = vert;
        current = next;
    }
    true
}

impl EditMesh {
    pub fn find_edge_loops(&self, test: impl Fn(EdgeId) -> bool) -> Vec<EdgeLoop> {
        find_edge_loops(self, test)
    }
}

fn find_edge_loops(mesh: &EditMesh, test: impl Fn(EdgeId) -> bool) -> Vec<EdgeLoop> {
    let mut vert_tags: IdSet<VertId> = IdSet::new();
    let mut edge_tags: IdSet<EdgeId> = IdSet::new();
    let mut edges = Vec::new();
    for edge in mesh.edges() {
        if test(edge) {
            edge_tags.insert(edge);
            let [a, b] = mesh.edge_verts(edge);
            vert_tags.insert(a);
            vert_tags.insert(b);
            edges.push(edge);
        }
    }
    let mut loops = Vec::new();
    for edge in edges {
        if !edge_tags.contains(edge) {
            continue;
        }
        let [a, b] = mesh.edge_verts(edge);
        let mut store = EdgeLoop::default();
        let ok = build(mesh, &mut store, a, b, true, &mut vert_tags, &mut edge_tags)
            && build(
                mesh,
                &mut store,
                b,
                a,
                false,
                &mut vert_tags,
                &mut edge_tags,
            );
        if ok && store.verts.len() > 1 {
            loops.push(store);
        }
    }
    loops
}

impl EdgeLoop {
    pub fn from_verts(verts: &[VertId], closed: bool) -> EdgeLoop {
        EdgeLoop {
            verts: verts.iter().copied().collect(),
            closed,
            center: Vec3::ZERO,
            normal: Vec3::ZERO,
        }
    }

    pub fn len(&self) -> usize {
        self.verts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.verts.is_empty()
    }

    pub fn first(&self) -> VertId {
        self.verts[0]
    }

    pub fn last(&self) -> VertId {
        self.verts[self.verts.len() - 1]
    }

    pub fn next_index(&self, index: usize) -> Option<usize> {
        if index + 1 < self.verts.len() {
            Some(index + 1)
        } else if self.closed {
            Some(0)
        } else {
            None
        }
    }

    pub fn edges(&self, mesh: &EditMesh) -> Vec<EdgeId> {
        let mut edges = Vec::with_capacity(self.verts.len());
        for index in 0..self.verts.len().saturating_sub(1) {
            if let Some(edge) = mesh.edge_between(self.verts[index], self.verts[index + 1]) {
                edges.push(edge);
            }
        }
        if self.closed
            && let Some(edge) = mesh.edge_between(self.first(), self.last())
        {
            edges.push(edge);
        }
        edges
    }

    pub fn calc_center(&mut self, mesh: &EditMesh) {
        let count = self.verts.len();
        let at = |index: usize| mesh.position(self.verts[index]);
        let mut center = Vec3::ZERO;
        let mut total = 0.0;
        let mut current = at(count - 1);
        let mut weight_previous = at((count + count - 2) % count).distance(current);
        let mut next_index = 0;
        loop {
            let next = at(next_index);
            let weight_current = current.distance(next);
            let weight = weight_current + weight_previous;
            center += current * weight;
            total += weight;
            weight_previous = weight_current;
            current = next;
            next_index += 1;
            if next_index >= count {
                break;
            }
        }
        if total != 0.0 {
            center *= 1.0 / total;
        }
        self.center = center;
    }

    pub fn calc_normal(&mut self, mesh: &EditMesh) -> bool {
        let mut normal = Vec3::ZERO;
        let mut previous = mesh.position(self.last());
        for &vert in &self.verts {
            let current = mesh.position(vert);
            normal += newell_cross(previous, current);
            previous = current;
        }
        self.finish_normal(normal)
    }

    pub fn calc_normal_aligned(&mut self, mesh: &EditMesh, align: Vec3) -> bool {
        let mut normal = Vec3::ZERO;
        let mut previous = mesh.position(self.last());
        for &vert in &self.verts {
            let current = mesh.position(vert);
            let direction = current - previous;
            let cross = align.cross(direction);
            normal += direction.cross(cross);
            previous = current;
        }
        self.finish_normal(normal)
    }

    fn finish_normal(&mut self, normal: Vec3) -> bool {
        let (normal, length) = normalize_with_length(normal);
        if length < EDGELOOP_EPS {
            self.normal = Vec3::Z;
            return false;
        }
        self.normal = normal;
        true
    }

    pub fn flip(&mut self) {
        self.normal = -self.normal;
        self.verts.make_contiguous().reverse();
    }

    pub fn rotate_first(&mut self, index: usize) {
        self.verts.rotate_left(index);
    }

    pub fn expand(&mut self, target: usize) {
        while self.verts.len() * 2 < target {
            let mut expanded = VecDeque::with_capacity(self.verts.len() * 2);
            for &vert in &self.verts {
                expanded.push_back(vert);
                expanded.push_back(vert);
            }
            self.verts = expanded;
        }
        let len = self.verts.len();
        if len < target {
            let picks = sparse_range(len, target - len);
            let mut expanded = VecDeque::with_capacity(target);
            let mut picks = picks.into_iter().peekable();
            for (index, &vert) in self.verts.iter().enumerate() {
                expanded.push_back(vert);
                if picks.peek() == Some(&index) {
                    picks.next();
                    expanded.push_back(vert);
                }
            }
            self.verts = expanded;
        }
    }

    pub fn expand_split(
        &mut self,
        mesh: &mut EditMesh,
        target: usize,
        split_edges: &mut Vec<(EdgeId, VertId)>,
    ) -> Result<()> {
        let mut swap = true;
        while self.len() * 2 < target {
            let mut index = 0;
            while index < self.len() {
                let current = self.verts[index];
                if index + 1 < self.len() || self.closed {
                    let other = self.verts[(index + 1) % self.len()];
                    let created = split_at(mesh, current, other, swap, split_edges)?;
                    self.verts.insert(index + 1, created);
                } else {
                    let other = self.verts[index - 1];
                    let created = split_at(mesh, current, other, swap, split_edges)?;
                    self.verts.insert(index, created);
                }
                index += 2;
                swap = !swap;
            }
            swap = !swap;
        }
        let len = self.len();
        if len < target {
            for (offset, pick) in sparse_range(len, target - len).into_iter().enumerate() {
                let index = pick + offset;
                let current = self.verts[index];
                if index + 1 < self.len() || self.closed {
                    let other = self.verts[(index + 1) % self.len()];
                    let created = split_at(mesh, current, other, swap, split_edges)?;
                    self.verts.insert(index + 1, created);
                } else {
                    let other = self.verts[index - 1];
                    let created = split_at(mesh, current, other, swap, split_edges)?;
                    self.verts.insert(index, created);
                }
                swap = !swap;
            }
        }
        Ok(())
    }

    pub fn calc_order(loops: &mut Vec<EdgeLoop>, use_normals: bool) {
        calc_order(loops, use_normals);
    }
}

fn split_at(
    mesh: &mut EditMesh,
    copy: VertId,
    other: VertId,
    swap: bool,
    split_edges: &mut Vec<(EdgeId, VertId)>,
) -> Result<VertId> {
    let Some(edge) = mesh.edge_between(copy, other) else {
        return Ok(copy);
    };
    let at = match swap {
        true => copy,
        false => other,
    };
    let split = mesh.split_edge(edge, at, 0.0)?;
    mesh.set_disk_start(split.vert, split.edge);
    split_edges.push((split.edge, split.vert));
    Ok(split.vert)
}

fn sparse_range(source: usize, picks: usize) -> Vec<usize> {
    let source = source as i64;
    let source2 = source * 2;
    let picks2 = picks as i64 * 2;
    let mut error = picks2 - source;
    let mut index: i64 = 0;
    let mut result = Vec::with_capacity(picks);
    loop {
        let delta = error.div_euclid(picks2);
        index -= delta;
        if index >= source {
            break;
        }
        result.push(index as usize);
        error -= delta * picks2 + source2;
    }
    result
}

fn calc_order(loops: &mut Vec<EdgeLoop>, use_normals: bool) {
    let mut center = Vec3::ZERO;
    let mut total = 0;
    for store in loops.iter() {
        if !store.center.is_finite() {
            continue;
        }
        center += store.center;
        total += 1;
    }
    if total > 0 {
        center *= 1.0 / total as f32;
        if !center.is_finite() {
            center = Vec3::ZERO;
        }
    }
    let mut ordered = Vec::with_capacity(loops.len());
    let mut best = 0;
    let mut best_length = -1.0f32;
    for (index, store) in loops.iter().enumerate() {
        let length = center.distance_squared(store.center);
        if length > best_length || index == 0 {
            best_length = length;
            best = index;
        }
    }
    ordered.push(loops.remove(best));
    while !loops.is_empty() {
        let last = ordered.last().expect("ordered loops start with one loop");
        let (co, no) = (last.center, last.normal);
        let mut best = 0;
        let mut best_length = f32::MAX;
        for (index, store) in loops.iter().enumerate() {
            let length = match use_normals {
                true => {
                    let (direction, length) = normalize_with_length(co - store.center);
                    length
                        * ((1.0 - direction.dot(no).abs())
                            + (1.0 - direction.dot(store.normal).abs()))
                }
                false => co.distance_squared(store.center),
            };
            if length < best_length || index == 0 {
                best_length = length;
                best = index;
            }
        }
        ordered.push(loops.remove(best));
    }
    *loops = ordered;
}
