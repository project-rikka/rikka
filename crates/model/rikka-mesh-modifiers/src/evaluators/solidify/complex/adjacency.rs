use std::f64::consts::TAU;

use glam::Vec3;
use rikka_polymesh::Polymesh;

use super::{EMPTY, NewEdge, Settings, Topology, normalize_length, other_side};
use crate::kinds::solidify::{Solidify, ThicknessMode};

struct AdjFaces {
    faces: Vec<u32>,
    reversed: Vec<bool>,
    used: u32,
}

struct Builder<'a> {
    mesh: &'a Polymesh,
    vm: Vec<u32>,
    coords: Vec<Vec3>,
    edge_lengths: Vec<f32>,
    adj_len: Vec<u32>,
    adj_ref: Vec<Option<u32>>,
    adj: Vec<AdjFaces>,
    vert_edges: Vec<Option<Vec<u32>>>,
    vert_edges_len: Vec<u32>,
}

fn contains(pair: [u32; 2], value: u32) -> bool {
    pair[0] == value || pair[1] == value
}

fn signed_angle(normal: Vec3, reference: Vec3, axis: Vec3) -> f32 {
    let angle = normal.dot(reference).clamp(-1.0, 1.0).acos();
    match normal.cross(reference).dot(axis) >= 0.0 {
        true => (TAU - f64::from(angle)) as f32,
        false => angle,
    }
}

pub(super) fn build(solidify: &Solidify, mesh: &Polymesh, settings: &Settings) -> Topology {
    let verts = mesh.point_count();
    let edges = mesh.edge_count();
    let faces = mesh.face_count();
    let constraints = solidify.nonmanifold_thickness_mode == ThicknessMode::Constraints;

    let mut face_normals = mesh.face_normals();
    let mut null_faces = vec![false; faces];
    for (face, normal) in face_normals.iter_mut().enumerate() {
        if normal.length_squared() < 0.5 {
            let [a, b] = mesh.edges[mesh.corner_edges[mesh.face(face).start] as usize];
            let direction = mesh.positions[b as usize] - mesh.positions[a as usize];
            match direction.z.abs() < direction.y.abs() {
                true => normal.z = 1.0,
                false => normal.y = 1.0,
            }
            if constraints {
                null_faces[face] = true;
            }
        }
    }

    let mut adj_len = vec![0u32; edges];
    for &edge in &mesh.corner_edges {
        adj_len[edge as usize] += 1;
    }

    let mut builder = Builder {
        mesh,
        vm: (0..verts as u32).collect(),
        coords: mesh.positions.clone(),
        edge_lengths: vec![0.0; edges],
        adj_len,
        adj_ref: vec![None; edges],
        adj: Vec::new(),
        vert_edges: vec![None; verts],
        vert_edges_len: vec![0; verts],
    };
    builder.link_faces();
    builder.merge_short_edges(solidify.nonmanifold_merge_threshold);
    builder.link_vert_edges();
    builder.remove_duplicate_faces();

    let mut topology = Topology {
        vm: Vec::new(),
        coords: Vec::new(),
        face_normals,
        null_faces,
        edge_lengths: Vec::new(),
        adj_len: Vec::new(),
        edges: Vec::new(),
        lists: Vec::new(),
        edge_list: vec![None; edges],
        side_links: (0..faces * 2)
            .map(|side| vec![None; mesh.face(side / 2).len()])
            .collect(),
        vert_edges: Vec::new(),
        groups: vec![Vec::new(); verts],
        has_singularities: false,
    };
    builder.new_edges(&mut topology, settings);
    topology.vm = builder.vm;
    topology.coords = builder.coords;
    topology.edge_lengths = builder.edge_lengths;
    topology.adj_len = builder.adj_len;
    topology.vert_edges = builder
        .vert_edges
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect();
    topology
}

impl Builder<'_> {
    fn link_faces(&mut self) {
        let mesh = self.mesh;
        for face in 0..mesh.face_count() {
            for corner in mesh.face(face) {
                let vert = mesh.corner_verts[corner];
                let edge = mesh.corner_edges[corner] as usize;
                let reversed = mesh.edges[edge][1] != vert;
                match self.adj_ref[edge] {
                    None => {
                        let len = self.adj_len[edge] as usize;
                        let mut faces = vec![EMPTY; len];
                        let mut flags = vec![false; len];
                        faces[0] = face as u32;
                        flags[0] = reversed;
                        self.adj_ref[edge] = Some(self.adj.len() as u32);
                        self.adj.push(AdjFaces {
                            faces,
                            reversed: flags,
                            used: 1,
                        });
                    }
                    Some(list) => {
                        let list = &mut self.adj[list as usize];
                        if let Some(slot) = (1..list.faces.len()).find(|&k| list.faces[k] == EMPTY)
                        {
                            list.faces[slot] = face as u32;
                            list.reversed[slot] = reversed;
                        }
                    }
                }
            }
        }
    }

    fn can_merge(&self, edge: usize, v1: u32, v2: u32) -> bool {
        let mesh = self.mesh;
        let vm = &self.vm;
        for other in 0..mesh.edge_count() {
            let [a, b] = mesh.edges[other];
            if other == edge
                || self.adj_len[other] == 0
                || contains([v1, v2], vm[a as usize]) == contains([v1, v2], vm[b as usize])
            {
                continue;
            }
            let Some(list) = self.adj_ref[other] else {
                continue;
            };
            for &face in &self.adj[list as usize].faces {
                let range = mesh.face(face as usize);
                let size = range.len();
                let mut changes = 0;
                let mut multiple_others = false;
                let mut unique_other = EMPTY;
                let mut current = size - 1;
                let mut next = 0;
                while next < size && changes <= 2 {
                    let current_vert = vm[mesh.corner_verts[range.start + current] as usize];
                    let next_vert = vm[mesh.corner_verts[range.start + next] as usize];
                    changes += u32::from(
                        contains([v1, v2], current_vert) != contains([v1, v2], next_vert),
                    );
                    if !contains([v1, v2], current_vert) {
                        if unique_other == EMPTY {
                            unique_other = current_vert;
                        } else if unique_other != current_vert {
                            multiple_others = true;
                        }
                    }
                    current = next;
                    next += 1;
                }
                if !(changes <= 2 && !(changes == 2 && !multiple_others)) {
                    return false;
                }
            }
        }
        true
    }

    fn merge_short_edges(&mut self, threshold: f32) {
        let mesh = self.mesh;
        let verts = mesh.point_count();
        let tolerance = threshold * threshold;
        let mut combined = vec![0u32; verts];
        for edge in 0..mesh.edge_count() {
            if self.adj_len[edge] == 0 {
                continue;
            }
            let [a, b] = mesh.edges[edge];
            let (mut v1, mut v2) = (self.vm[a as usize], self.vm[b as usize]);
            if v1 == v2 {
                continue;
            }
            if v2 < v1 {
                std::mem::swap(&mut v1, &mut v2);
            }
            let direction = self.coords[v2 as usize] - self.coords[v1 as usize];
            let squared = direction.length_squared();
            self.edge_lengths[edge] = squared;
            if squared <= tolerance {
                if !self.can_merge(edge, v1, v2) {
                    self.edge_lengths[edge] = 0.0;
                    self.vert_edges_len[v1 as usize] += 1;
                    self.vert_edges_len[v2 as usize] += 1;
                    continue;
                }
                let share = (combined[v2 as usize] + 1) as f32
                    / (combined[v1 as usize] + combined[v2 as usize] + 2) as f32;
                self.coords[v1 as usize] += direction * share;
                for slot in &mut self.vm[v2 as usize..] {
                    if *slot == v2 {
                        *slot = v1;
                    }
                }
                self.vert_edges_len[v1 as usize] += self.vert_edges_len[v2 as usize];
                self.vert_edges_len[v2 as usize] = 0;
                combined[v1 as usize] += combined[v2 as usize] + 1;
                self.adj_len[edge] = 0;
                self.adj_ref[edge] = None;
            } else {
                self.edge_lengths[edge] = squared.sqrt();
                self.vert_edges_len[v1 as usize] += 1;
                self.vert_edges_len[v2 as usize] += 1;
            }
        }
        for edge in 0..mesh.edge_count() {
            let [a, b] = mesh.edges[edge];
            if self.vm[a as usize] == self.vm[b as usize] {
                self.adj_len[edge] = 0;
                self.adj_ref[edge] = None;
            }
        }
    }

    fn link_vert_edges(&mut self) {
        let mesh = self.mesh;
        let mut edge = 0;
        while edge < mesh.edge_count() {
            if self.adj_len[edge] > 0 {
                let [a, b] = mesh.edges[edge];
                let ends = [self.vm[a as usize], self.vm[b as usize]];
                let mut invalid = None;
                for side in 0..2 {
                    let vert = ends[side] as usize;
                    let capacity = self.vert_edges_len[vert] as usize;
                    if capacity == 0 {
                        continue;
                    }
                    let Some(list) = self.vert_edges[vert].as_mut() else {
                        self.vert_edges[vert] = Some(vec![edge as u32]);
                        continue;
                    };
                    let mut slot = 0;
                    while slot < capacity && slot <= list.len() {
                        if slot == list.len() {
                            list.push(edge as u32);
                            break;
                        }
                        let [c, d] = mesh.edges[list[slot] as usize];
                        if self.vm[c as usize] == ends[1 - side] {
                            invalid = Some((list[slot] as usize, side == 0));
                            break;
                        }
                        if self.vm[d as usize] == ends[1 - side] {
                            invalid = Some((list[slot] as usize, side == 1));
                            break;
                        }
                        slot += 1;
                    }
                    if invalid.is_some() {
                        if side == 1
                            && let Some(first) = self.vert_edges[ends[0] as usize].as_mut()
                        {
                            first.pop();
                        }
                        break;
                    }
                }
                if let Some((kept, reversed)) = invalid {
                    self.merge_edge_pair(kept, edge, reversed);
                }
            }
            edge += 1;
        }
    }

    fn merge_edge_pair(&mut self, kept: usize, removed: usize, reversed: bool) {
        let (Some(kept_list), Some(removed_list)) = (self.adj_ref[kept], self.adj_ref[removed])
        else {
            return;
        };
        if kept_list == removed_list {
            return;
        }
        let (kept_list, removed_list) = (kept_list as usize, removed_list as usize);
        let mut kept_faces = self.adj[kept_list].faces.clone();
        let mut removed_faces = self.adj[removed_list].faces.clone();
        for kept in kept_faces.iter_mut() {
            for removed in removed_faces.iter_mut() {
                if *kept == *removed && *kept != EMPTY {
                    *kept = EMPTY;
                    *removed = EMPTY;
                }
            }
        }
        let mut faces = Vec::new();
        let mut flags = Vec::new();
        for (k, &face) in kept_faces.iter().enumerate() {
            if face != EMPTY {
                faces.push(face);
                flags.push(self.adj[kept_list].reversed[k]);
            }
        }
        for (l, &face) in removed_faces.iter().enumerate() {
            if face != EMPTY {
                faces.push(face);
                flags.push(reversed != self.adj[removed_list].reversed[l]);
            }
        }
        self.adj_len[removed] = 0;
        self.adj_len[kept] = faces.len() as u32;
        let used = self.adj[removed_list].used;
        let list = &mut self.adj[kept_list];
        list.faces = faces;
        list.reversed = flags;
        list.used += used;
        self.adj_ref[removed] = Some(kept_list as u32);
    }

    fn remove_duplicate_faces(&mut self) {
        let mesh = self.mesh;
        for edge in 0..mesh.edge_count() {
            if self.adj_len[edge] == 0 {
                continue;
            }
            let Some(list) = self.adj_ref[edge] else {
                continue;
            };
            let list = list as usize;
            let mut adj_len = self.adj[list].faces.len();
            if adj_len <= 1 {
                continue;
            }
            let mut j = 0;
            while j < adj_len {
                if (j + 1..adj_len).any(|k| self.same_face(list, j, k)) {
                    adj_len -= self.remove_face(list, j);
                }
                j += 1;
            }
        }
    }

    fn same_face(&self, list: usize, j: usize, k: usize) -> bool {
        let mesh = self.mesh;
        let adj = &self.adj[list];
        let j_range = mesh.face(adj.faces[j] as usize);
        let k_range = mesh.face(adj.faces[k] as usize);
        let total = j_range.len();
        if k_range.len() != total {
            return false;
        }
        let j_first = self.vm[mesh.corner_verts[j_range.start] as usize];
        let Some(l) =
            (0..total).find(|&l| self.vm[mesh.corner_verts[k_range.start + l] as usize] == j_first)
        else {
            return false;
        };
        let reversed = adj.reversed[j] != adj.reversed[k];
        (0..total).all(|m| {
            let n = match reversed {
                true => (l + total * 2 - m) % total,
                false => (l + m) % total,
            };
            self.vm[mesh.corner_verts[j_range.start + m] as usize]
                == self.vm[mesh.corner_verts[k_range.start + n] as usize]
        })
    }

    fn remove_face(&mut self, list: usize, j: usize) -> usize {
        let mesh = self.mesh;
        let face = self.adj[list].faces[j];
        let mut shrunk = 0;
        for corner in mesh.face(face as usize) {
            let other = mesh.corner_edges[corner] as usize;
            let Some(other_list) = self.adj_ref[other] else {
                continue;
            };
            let other_list = other_list as usize;
            let position = match other_list == list {
                true => {
                    shrunk += 1;
                    j
                }
                false => match self.adj[other_list]
                    .faces
                    .iter()
                    .position(|&candidate| candidate == face)
                {
                    Some(position) => position,
                    None => continue,
                },
            };
            if position < self.adj[other_list].faces.len() {
                self.adj[other_list].faces.remove(position);
                self.adj[other_list].reversed.remove(position);
            }
            if self.adj_len[other] > 0 {
                self.adj_len[other] -= 1;
                if self.adj_len[other] == 0 {
                    self.adj[other_list].used -= 1;
                    self.adj_ref[other] = None;
                }
            } else if self.adj[other_list].used > 1 {
                let shared = (0..mesh.edge_count()).find(|&candidate| {
                    self.adj_ref[candidate] == Some(other_list as u32)
                        && self.adj_len[candidate] > 0
                });
                if let Some(candidate) = shared {
                    self.adj_len[candidate] -= 1;
                    if self.adj_len[candidate] == 0 {
                        self.adj[other_list].used -= 1;
                        self.adj_ref[candidate] = None;
                    }
                }
            }
        }
        shrunk
    }

    fn fallback_direction(&self, edge: usize, v1: u32, v2: u32) -> Vec3 {
        let mesh = self.mesh;
        let origin = self.coords[v2 as usize];
        let sum = |vert: u32| {
            let mut direction = Vec3::ZERO;
            for &other in self.vert_edges[vert as usize].iter().flatten() {
                let other = other as usize;
                if self.adj_len[other] == 0 || other == edge {
                    continue;
                }
                let [a, b] = mesh.edges[other];
                let far = match self.vm[a as usize] == vert {
                    true => b,
                    false => a,
                };
                direction += self.coords[self.vm[far as usize] as usize] - origin;
            }
            direction
        };
        let (direction, length) = normalize_length(sum(v2) - sum(v1));
        match length == 0.0 {
            true => Vec3::Z,
            false => direction,
        }
    }

    fn new_edges(&self, topology: &mut Topology, settings: &Settings) {
        let mesh = self.mesh;
        for edge in 0..mesh.edge_count() {
            if self.adj_len[edge] == 0 {
                continue;
            }
            let Some(list) = self.adj_ref[edge] else {
                continue;
            };
            let list = &self.adj[list as usize];
            let [a, b] = mesh.edges[edge];
            let (v1, v2) = (self.vm[a as usize], self.vm[b as usize]);
            let direction = match self.edge_lengths[edge] > f32::EPSILON {
                true => {
                    (self.coords[v2 as usize] - self.coords[v1 as usize])
                        * (1.0 / self.edge_lengths[edge])
                }
                false => self.fallback_direction(edge, v1, v2),
            };
            let adj_len = list.faces.len();
            let side_of = |slot: usize| list.faces[slot] * 2 + u32::from(list.reversed[slot]);
            let mut sorted: Vec<(f32, u32)> = Vec::with_capacity(adj_len);
            let count = if adj_len > 1 {
                let mut reference = Vec3::ZERO;
                for slot in 0..adj_len {
                    let face = list.faces[slot] as usize;
                    let mut normal = match list.reversed[slot] {
                        true => -topology.face_normals[face],
                        false => topology.face_normals[face],
                    };
                    let mut length = 1.0;
                    if mesh.face(face).len() > 3 {
                        let along = normal.dot(direction);
                        normal -= direction * along;
                        length = match along != 0.0 {
                            true => {
                                let (normalized, length) = normalize_length(normal);
                                normal = normalized;
                                length
                            }
                            false => 1.0,
                        };
                    }
                    let angle = if length == 0.0 {
                        0.0
                    } else if slot == 0 {
                        reference = normal;
                        0.0
                    } else {
                        -signed_angle(normal, reference, direction)
                    };
                    sorted.push((angle, side_of(slot)));
                }
                sorted.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
                adj_len
            } else {
                sorted.push((0.0, side_of(0)));
                2
            };
            let list_index = topology.lists.len() as u32;
            let mut created = Vec::with_capacity(count);
            for j in 0..count {
                let (faces, angle) = if adj_len > 1 {
                    let next = (j + 1) % adj_len;
                    let mut angle = sorted[next].0 - sorted[j].0;
                    if angle < 0.0 {
                        angle = (f64::from(angle) + TAU) as f32;
                    }
                    ([Some(sorted[j].1), Some(other_side(sorted[next].1))], angle)
                } else {
                    ([Some(sorted[0].1 ^ j as u32), None], 0.0)
                };
                let new_edge = match settings.shell || (adj_len == 1 && settings.rim) {
                    true => 0,
                    false => super::EMPTY,
                };
                let id = topology.edges.len() as u32;
                topology.edges.push(NewEdge {
                    old_edge: edge as u32,
                    faces,
                    groups: [None, None],
                    angle,
                    new_edge,
                });
                created.push(id);
                for side in faces.into_iter().flatten() {
                    let range = mesh.face(side as usize / 2);
                    for (corner, slot) in range.clone().zip(0..) {
                        let other = mesh.corner_edges[corner] as usize;
                        if self.adj_ref[other] == self.adj_ref[edge] {
                            if other != edge && topology.edge_list[other].is_none() {
                                topology.edge_list[other] = Some(list_index);
                            }
                            topology.side_links[side as usize][slot] = Some(id);
                            break;
                        }
                    }
                }
            }
            topology.lists.push(created);
            topology.edge_list[edge] = Some(list_index);
        }
    }
}
