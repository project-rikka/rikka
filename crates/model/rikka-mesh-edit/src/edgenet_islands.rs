use glam::Vec2;

use crate::error::Result;
use crate::ids::{EdgeId, FaceId, VertId};
use crate::math::{axis_dominant, cross_2d, intersect_segments_2d, line_point_factor_2d};
use crate::mesh::EditMesh;
use crate::sets::{IdMap, IdSet};

const ENDPOINT_BIAS: f32 = 1.0e-4;

#[derive(Clone, Debug)]
struct LocalVert {
    mesh: VertId,
    disk: Vec<EdgeId>,
    origin: Option<usize>,
}

struct Islands<'a> {
    mesh: &'a EditMesh,
    face: FaceId,
    verts: Vec<LocalVert>,
    of_mesh: IdMap<VertId, usize>,
    ends: IdMap<EdgeId, [usize; 2]>,
    edge_flag: IdSet<EdgeId>,
    vert_flag: Vec<bool>,
}

impl Islands<'_> {
    fn vert(&mut self, vert: VertId) -> usize {
        if let Some(index) = self.of_mesh.copied(vert) {
            return index;
        }
        let disk = self.mesh.vert_edges(vert).collect();
        self.verts.push(LocalVert {
            mesh: vert,
            disk,
            origin: None,
        });
        self.vert_flag.push(false);
        let index = self.verts.len() - 1;
        self.of_mesh.insert(vert, index);
        index
    }

    fn ends(&mut self, edge: EdgeId) -> [usize; 2] {
        if let Some(ends) = self.ends.copied(edge) {
            return ends;
        }
        let [a, b] = self.mesh.edge_verts(edge);
        let ends = [self.vert(a), self.vert(b)];
        self.ends.insert(edge, ends);
        ends
    }

    fn other(&mut self, edge: EdgeId, vert: usize) -> usize {
        let [a, b] = self.ends(edge);
        if a == vert { b } else { a }
    }

    fn in_face(&self, edge: EdgeId) -> bool {
        self.mesh.is_edge_in_face(edge, self.face)
    }

    fn partial_connect(&mut self, delimit: usize) -> Option<usize> {
        let disk = self.verts[delimit].disk.clone();
        let mut delimit_list: Vec<EdgeId> = Vec::new();
        let mut face_init = None;
        for &edge in &disk {
            if self.edge_flag.contains(edge) {
                delimit_list.push(edge);
                if self.mesh.edge_first_loop(edge).is_some() && self.in_face(edge) {
                    face_init = Some(edge);
                }
            }
        }
        if delimit_list.len() <= 2 {
            return None;
        }
        let mut stack_verts = vec![delimit];
        self.vert_flag[delimit] = false;
        let start_edge = face_init.or_else(|| disk.first().copied())?;
        let mut other = self.other(start_edge, delimit);
        let mut search = vec![other];
        if self.vert_flag[other] {
            self.vert_flag[other] = false;
            stack_verts.push(other);
        }
        while let Some(current) = search.pop() {
            other = current;
            let edges = self.verts[other].disk.clone();
            for edge in edges {
                let step = self.other(edge, other);
                if self.edge_flag.contains(edge) && self.vert_flag[step] {
                    self.vert_flag[step] = false;
                    search.push(step);
                    stack_verts.push(step);
                }
            }
        }
        let mut is_delimit = false;
        for &edge in &disk {
            let step = self.other(edge, delimit);
            if self.vert_flag[step] && !self.in_face(edge) {
                is_delimit = true;
            } else {
                self.edge_flag.remove(edge);
            }
        }
        let mut split = None;
        if is_delimit {
            let tested: Vec<EdgeId> = disk
                .iter()
                .copied()
                .filter(|&edge| self.edge_flag.contains(edge) && !self.in_face(edge))
                .collect();
            let index = self.verts.len();
            self.verts.push(LocalVert {
                mesh: self.verts[delimit].mesh,
                disk: Vec::new(),
                origin: Some(delimit),
            });
            self.vert_flag.push(true);
            for &edge in tested.iter().rev() {
                self.verts[delimit].disk.retain(|&other| other != edge);
                self.verts[index].disk.push(edge);
                let mut ends = self.ends(edge);
                for end in &mut ends {
                    if *end == delimit {
                        *end = index;
                    }
                }
                self.ends.insert(edge, ends);
            }
            if self.verts[index].disk.is_empty() {
                self.verts.pop();
                self.vert_flag.pop();
            } else {
                split = Some(index);
            }
        }
        for vert in stack_verts {
            self.vert_flag[vert] = true;
        }
        for edge in delimit_list {
            self.edge_flag.insert(edge);
        }
        split
    }
}

struct Group {
    edges: Vec<EdgeId>,
    vert_len: usize,
    min: usize,
    max: usize,
    min_axis: Vec2,
    max_axis: Vec2,
    has_prev_edge: bool,
}

fn axis_cmp(a: Vec2, b: Vec2) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if a.x < b.x {
        return Ordering::Less;
    }
    if a.x > b.x {
        return Ordering::Greater;
    }
    if a.y < b.y {
        return Ordering::Less;
    }
    if a.y > b.y {
        return Ordering::Greater;
    }
    Ordering::Equal
}

fn ray_segment(origin: Vec2, direction: Vec2, v0: Vec2, v1: Vec2) -> Option<f32> {
    let a = v0 - origin;
    let b = v1 - origin;
    let s10 = b - a;
    let det = cross_2d(direction, s10);
    if det == 0.0 {
        return None;
    }
    let v = cross_2d(a, b);
    let p = Vec2::new(direction.x * v / det, direction.y * v / det);
    let t = p.dot(direction) / direction.dot(direction);
    if t < 0.0 || t.is_nan() {
        return None;
    }
    let h = b - p;
    let u = s10.dot(h) / s10.dot(s10);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    Some(t)
}

struct Search<'a> {
    coords: &'a [Vec2],
    index: &'a [usize],
    edges: &'a [(EdgeId, [usize; 2])],
    new_edges: &'a [[usize; 2]],
    range: [usize; 2],
}

impl Search<'_> {
    fn co(&self, local: usize) -> Vec2 {
        self.coords[self.index[local]]
    }

    fn outside(&self, local: usize) -> bool {
        let index = self.index[local];
        index < self.range[0] || index >= self.range[1]
    }

    fn isect_verts(&self, ends: [usize; 2], a: usize, b: usize) -> Option<Vec2> {
        if ends.contains(&a) || ends.contains(&b) {
            return None;
        }
        intersect_segments_2d(
            self.co(a),
            self.co(b),
            self.co(ends[0]),
            self.co(ends[1]),
            ENDPOINT_BIAS,
        )
    }

    fn test_vert(&self, origin: usize, other: usize) -> Option<[usize; 2]> {
        let (o, t) = (self.co(origin), self.co(other));
        let distance = (t - o).length();
        let mut best = distance;
        let mut hit = None;
        for &(_, ends) in self.edges {
            if let Some(point) = self.isect_verts(ends, origin, other) {
                let factor = line_point_factor_2d(point, o, t);
                let candidate = distance * factor;
                if candidate < best && candidate > 0.0 && self.outside(ends[0]) {
                    best = candidate;
                    hit = Some(ends);
                }
            }
        }
        if hit.is_none() {
            let mut best_factor = 1.0;
            for &ends in self.new_edges {
                if let Some(point) = self.isect_verts(ends, origin, other) {
                    let factor = line_point_factor_2d(point, o, t);
                    if factor < best_factor {
                        best_factor = factor;
                        hit = Some(ends);
                    }
                }
            }
        }
        hit
    }

    fn test_ray(&self, origin: usize, direction: Vec2) -> Option<[usize; 2]> {
        let o = self.co(origin);
        let mut best = f32::MAX;
        let mut hit = None;
        for &(_, ends) in self.edges {
            if let Some(distance) = ray_segment(o, direction, self.co(ends[0]), self.co(ends[1]))
                && distance < best
                && distance > 0.0
                && !ends.contains(&origin)
                && self.outside(ends[0])
            {
                best = distance;
                hit = Some(ends);
            }
        }
        if hit.is_some() {
            for &ends in self.new_edges {
                if let Some(distance) =
                    ray_segment(o, direction, self.co(ends[0]), self.co(ends[1]))
                    && !ends.contains(&origin)
                    && distance < best
                {
                    best = distance;
                    hit = Some(ends);
                }
            }
        }
        hit
    }

    fn find_connection(&self, origin: usize, positive: bool) -> Option<usize> {
        let direction = Vec2::new(if positive { 1.0 } else { -1.0 }, 0.0);
        let mut hit = self.test_ray(origin, direction)?;
        let o = self.co(origin);
        let mut blocked: Vec<usize> = Vec::new();
        let mut search: Vec<usize> = Vec::new();
        let mut current: Option<usize> = None;
        loop {
            let pair =
                match o.distance_squared(self.co(hit[0])) > o.distance_squared(self.co(hit[1])) {
                    true => [hit[0], hit[1]],
                    false => [hit[1], hit[0]],
                };
            for vert in pair {
                if blocked.contains(&vert) {
                    continue;
                }
                let x = self.co(vert).x;
                if (positive && x > o.x) || (!positive && x < o.x) {
                    search.push(vert);
                    blocked.push(vert);
                }
            }
            let fallback = current;
            current = search.pop();
            let Some(other) = current else {
                return fallback;
            };
            match self.test_vert(origin, other) {
                Some(next) => hit = next,
                None => return Some(other),
            }
        }
    }
}

impl EditMesh {
    pub fn split_face_edgenet_connect_islands(
        &mut self,
        face: FaceId,
        edge_net: &[EdgeId],
        use_partial_connect: bool,
    ) -> Result<Option<Vec<EdgeId>>> {
        connect_islands(self, face, edge_net, use_partial_connect)
    }
}

fn connect_islands(
    mesh: &mut EditMesh,
    face: FaceId,
    edge_net: &[EdgeId],
    use_partial_connect: bool,
) -> Result<Option<Vec<EdgeId>>> {
    if edge_net.is_empty() || !mesh.contains_face(face) {
        return Ok(None);
    }
    let mut edge_arr: Vec<EdgeId> = edge_net.to_vec();
    edge_arr.extend(mesh.face_edges_vec(face));
    let normal = mesh.face_normal(face);
    let reference = mesh.position(mesh.loop_vert(mesh.face_first_loop(face)));
    let (groups, local, temp_pairs) = {
        let mut islands = Islands {
            mesh,
            face,
            verts: Vec::new(),
            of_mesh: IdMap::new(),
            ends: IdMap::new(),
            edge_flag: IdSet::new(),
            vert_flag: Vec::new(),
        };
        for &edge in &edge_arr {
            islands.edge_flag.insert(edge);
            let [a, b] = islands.ends(edge);
            islands.vert_flag[a] = true;
            islands.vert_flag[b] = true;
        }
        let mut temp_pairs: Vec<(usize, usize)> = Vec::new();
        if use_partial_connect {
            for &edge in edge_net {
                for side in 0..2 {
                    let delimit = islands.ends(edge)[side];
                    while let Some(split) = islands.partial_connect(delimit) {
                        temp_pairs.push((split, delimit));
                    }
                }
            }
        }
        let mut groups: Vec<(Vec<EdgeId>, usize)> = Vec::new();
        let mut edge_index = edge_arr.len() - 1;
        let mut total = 0;
        loop {
            let mut links: Vec<EdgeId> = Vec::new();
            let mut vert_count = 0;
            let start = islands.ends(edge_arr[edge_index])[0];
            islands.vert_flag[start] = false;
            let mut stack = vec![start];
            while let Some(vert) = stack.pop() {
                vert_count += 1;
                let disk = islands.verts[vert].disk.clone();
                for edge in disk {
                    if islands.edge_flag.contains(edge) {
                        islands.edge_flag.remove(edge);
                        links.push(edge);
                        let other = islands.other(edge, vert);
                        if islands.vert_flag[other] {
                            stack.push(other);
                            islands.vert_flag[other] = false;
                        }
                    }
                }
            }
            links.reverse();
            total += links.len();
            groups.push((links, vert_count));
            if total >= edge_arr.len() {
                break;
            }
            while !islands.edge_flag.contains(edge_arr[edge_index]) {
                if edge_index == 0 {
                    break;
                }
                edge_index -= 1;
            }
            if !islands.edge_flag.contains(edge_arr[edge_index]) {
                break;
            }
        }
        (groups, (islands.verts, islands.ends), temp_pairs)
    };
    if groups.len() <= 1 {
        return Ok(None);
    }
    let (local_verts, ends) = local;
    let matrix = axis_dominant(normal);
    let row_x = matrix.row(0);
    let row_y = matrix.row(1);
    let mut built: Vec<Group> = groups
        .into_iter()
        .map(|(edges, vert_len)| {
            let first = ends.copied(edges[0]).unwrap_or([0, 0]);
            let mut group = Group {
                edges,
                vert_len,
                min: first[0],
                max: first[1],
                min_axis: Vec2::splat(f32::MAX),
                max_axis: Vec2::splat(-f32::MAX),
                has_prev_edge: false,
            };
            for &edge in &group.edges {
                for vert in ends.copied(edge).unwrap_or([0, 0]) {
                    let co = mesh.position(local_verts[vert].mesh);
                    let value = Vec2::new(row_x.dot(co), row_y.dot(co));
                    if axis_cmp(value, group.min_axis) == std::cmp::Ordering::Less {
                        group.min = vert;
                        group.min_axis = value;
                    }
                    if axis_cmp(value, group.max_axis) == std::cmp::Ordering::Greater {
                        group.max = vert;
                        group.max_axis = value;
                    }
                }
            }
            group
        })
        .collect();
    built.sort_by(|a, b| axis_cmp(a.min_axis, b.min_axis).then(axis_cmp(a.max_axis, b.max_axis)));
    let mut index: Vec<usize> = vec![usize::MAX; local_verts.len()];
    let mut coords: Vec<Vec2> = Vec::new();
    let mut group_of: Vec<usize> = Vec::new();
    for (group_index, group) in built.iter().enumerate() {
        for &edge in &group.edges {
            for vert in ends.copied(edge).unwrap_or([0, 0]) {
                if index[vert] == usize::MAX {
                    index[vert] = coords.len();
                    let co = mesh.position(local_verts[vert].mesh) - reference;
                    coords.push(Vec2::new(row_x.dot(co), row_y.dot(co)));
                    group_of.push(group_index);
                }
            }
        }
    }
    let edge_list: Vec<(EdgeId, [usize; 2])> = edge_arr
        .iter()
        .map(|&edge| (edge, ends.copied(edge).unwrap_or([0, 0])))
        .collect();
    let mut remap: Vec<Option<usize>> = vec![None; coords.len()];
    if use_partial_connect {
        for &(temp, original) in &temp_pairs {
            if index[temp] != usize::MAX && index[original] != usize::MAX {
                remap[index[temp]] = Some(index[original]);
            }
        }
    }
    let use_partial_connect = use_partial_connect && !temp_pairs.is_empty();
    let overlaps = |a: usize, b: usize| remap[a] == Some(b) || remap[b] == Some(a);
    let real = |vert: usize| -> usize {
        let mut current = vert;
        while let Some(origin) = local_verts[current].origin {
            current = origin;
        }
        current
    };
    let mut new_net: Vec<EdgeId> = edge_net.to_vec();
    let mut new_ends: Vec<[usize; 2]> = Vec::new();
    let mut range = [0usize, built[0].vert_len];
    for group_index in 1..built.len() {
        range[0] = range[1];
        range[1] += built[group_index].vert_len;
        for positive in [false, true] {
            if !positive && built[group_index].has_prev_edge {
                continue;
            }
            let origin = if positive {
                built[group_index].max
            } else {
                built[group_index].min
            };
            let search = Search {
                coords: &coords,
                index: &index,
                edges: &edge_list,
                new_edges: &new_ends,
                range,
            };
            let Some(other) = search.find_connection(origin, positive) else {
                continue;
            };
            let other_index = index[other];
            if !(use_partial_connect && overlaps(index[origin], other_index)) {
                let a = local_verts[real(origin)].mesh;
                let b = local_verts[real(other)].mesh;
                let is_virtual = real(origin) != origin || real(other) != other;
                let existing = mesh.edge_between(a, b);
                match existing {
                    Some(_) if is_virtual => {}
                    Some(edge) => {
                        new_net.push(edge);
                        new_ends.push([origin, other]);
                    }
                    None if a != b => {
                        let edge = mesh.add_edge(a, b)?;
                        new_net.push(edge);
                        new_ends.push([origin, other]);
                    }
                    None => {}
                }
            }
            if positive {
                let target = group_of[other_index];
                built[target].has_prev_edge = true;
            }
        }
    }
    Ok(Some(new_net))
}
