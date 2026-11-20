use rikka_polymesh::{Domain, Polymesh, ordered};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Param {
    face: u32,
    patch: u32,
    u: u32,
    v: u32,
}

impl Param {
    fn mid(self, other: Param) -> Param {
        Param {
            u: (self.u + other.u) / 2,
            v: (self.v + other.v) / 2,
            ..self
        }
    }

    fn center(corners: &[Param; 4]) -> Param {
        Param {
            u: (corners[0].u + corners[1].u + corners[2].u + corners[3].u) / 4,
            v: (corners[0].v + corners[1].v + corners[2].v + corners[3].v) / 4,
            ..corners[0]
        }
    }
}

struct Coarse {
    points: u32,
    edges: Vec<[u32; 2]>,
    face_offsets: Vec<u32>,
    corner_verts: Vec<u32>,
    corner_edges: Vec<u32>,
}

impl Coarse {
    fn face_len(&self, face: usize) -> usize {
        (self.face_offsets[face + 1] - self.face_offsets[face]) as usize
    }

    fn corner(&self, face: usize, slot: usize) -> usize {
        self.face_offsets[face] as usize + slot
    }
}

pub(super) struct Topology<'a> {
    pub(super) points: usize,
    pub(super) faces: &'a [u32],
    pub(super) edges: &'a [[u32; 2]],
}

pub(super) struct Tracker {
    coarse: Coarse,
    levels: u32,
    faces: Vec<[Param; 4]>,
    edges: Vec<Option<(u32, u32, u32)>>,
    points: Vec<Option<(u32, u32)>>,
}

fn rotated(mut corners: [Param; 4], slot: usize) -> [Param; 4] {
    corners.rotate_right(slot);
    corners
}

impl Tracker {
    pub(super) fn new(mesh: &Polymesh, levels: u32) -> Tracker {
        let resolution = 1u32 << levels;
        Tracker {
            coarse: Coarse {
                points: mesh.point_count() as u32,
                edges: mesh.edges.clone(),
                face_offsets: mesh.face_offsets.clone(),
                corner_verts: mesh.corner_verts.clone(),
                corner_edges: mesh.corner_edges.clone(),
            },
            levels,
            faces: Vec::new(),
            edges: (0..mesh.edge_count() as u32)
                .map(|edge| Some((edge, 0, resolution)))
                .collect(),
            points: vec![None; mesh.point_count()],
        }
    }

    pub(super) fn refine(&mut self, before: &Topology<'_>) {
        let full = 1u32 << self.levels;
        let half = full / 2;
        let mut faces = Vec::with_capacity(before.faces.last().copied().unwrap_or(0) as usize);
        let face_count = before.faces.len() - 1;
        for face in 0..face_count {
            let size = (before.faces[face + 1] - before.faces[face]) as usize;
            for slot in 0..size {
                let corners = match self.faces.get(face) {
                    Some(parent) => {
                        let previous = (slot + 3) % 4;
                        let next = (slot + 1) % 4;
                        rotated(
                            [
                                parent[slot],
                                parent[slot].mid(parent[next]),
                                Param::center(parent),
                                parent[previous].mid(parent[slot]),
                            ],
                            slot,
                        )
                    }
                    None if size == 4 => {
                        let at = |u: u32, v: u32| Param {
                            face: face as u32,
                            patch: 0,
                            u,
                            v,
                        };
                        let quad = [at(0, 0), at(full, 0), at(full, full), at(0, full)];
                        rotated(
                            [
                                quad[slot],
                                quad[slot].mid(quad[(slot + 1) % 4]),
                                at(half, half),
                                quad[(slot + 3) % 4].mid(quad[slot]),
                            ],
                            slot,
                        )
                    }
                    None => {
                        let quarter = half;
                        let at = |u: u32, v: u32| Param {
                            face: face as u32,
                            patch: slot as u32,
                            u,
                            v,
                        };
                        [
                            at(0, 0),
                            at(quarter, 0),
                            at(quarter, quarter),
                            at(0, quarter),
                        ]
                    }
                };
                faces.push(corners);
            }
        }
        let corners = before.faces.last().copied().unwrap_or(0) as usize;
        let mut edges = Vec::with_capacity(before.edges.len() * 2 + corners);
        let mut points = self.points.clone();
        points.resize(before.points, None);
        for edge in 0..before.edges.len() {
            match self.edges.get(edge).copied().flatten() {
                Some((coarse, start, end)) => {
                    let middle = (start + end) / 2;
                    edges.push(Some((coarse, start, middle)));
                    edges.push(Some((coarse, middle, end)));
                    points.push(Some((coarse, middle)));
                }
                None => {
                    edges.push(None);
                    edges.push(None);
                    points.push(None);
                }
            }
        }
        edges.resize(edges.len() + corners, None);
        points.resize(points.len() + face_count, None);
        self.faces = faces;
        self.edges = edges;
        self.points = points;
    }

    pub(super) fn reorder(&self, mesh: &mut Polymesh) -> Option<Vec<u32>> {
        let order = self.order(mesh)?;
        mesh.gather(Domain::Point, &order.points);
        mesh.gather(Domain::Edge, &order.edges);
        mesh.edges = order.edge_verts;
        mesh.gather(Domain::Face, &order.faces);
        mesh.gather(Domain::Corner, &order.corners);
        mesh.face_offsets = (0..=order.faces.len() as u32)
            .map(|face| face * 4)
            .collect();
        mesh.corner_verts = order.corner_verts;
        mesh.corner_edges = order.corner_edges;
        Some(order.points)
    }

    fn order(&self, mesh: &Polymesh) -> Option<Order> {
        let layout = Layout::new(&self.coarse, self.levels);
        let mut blender_of = vec![u32::MAX; mesh.point_count()];
        for (point, slot) in blender_of.iter_mut().enumerate() {
            if (point as u32) < self.coarse.points {
                *slot = point as u32;
            } else if let Some((edge, along)) = self.points.get(point).copied().flatten() {
                *slot = layout.edge_vertex(edge, along);
            }
        }
        for face in 0..mesh.face_count() {
            for (slot, corner) in mesh.face(face).enumerate() {
                let point = mesh.corner_verts[corner] as usize;
                if blender_of[point] == u32::MAX {
                    blender_of[point] = layout.inner_vertex(self.faces.get(face)?[slot])?;
                }
            }
        }
        let mut points = vec![u32::MAX; mesh.point_count()];
        for (point, &blender) in blender_of.iter().enumerate() {
            let slot = points.get_mut(blender as usize)?;
            if *slot != u32::MAX {
                return None;
            }
            *slot = point as u32;
        }
        if points.contains(&u32::MAX) {
            return None;
        }
        let edge_verts = layout.edges();
        if edge_verts.len() != mesh.edge_count() {
            return None;
        }
        let groups = mesh.point_count();
        let ours = Buckets::new(
            groups,
            mesh.edges.iter().enumerate().map(|(edge, &[a, b])| {
                let [low, high] = ordered([blender_of[a as usize], blender_of[b as usize]]);
                (low, high, edge as u32)
            }),
        );
        let lookup = Buckets::new(
            groups,
            edge_verts.iter().enumerate().map(|(index, &pair)| {
                let [low, high] = ordered(pair);
                (low, high, index as u32)
            }),
        );
        let mut edges = Vec::with_capacity(edge_verts.len());
        for &pair in &edge_verts {
            let [low, high] = ordered(pair);
            edges.push(ours.last(low, high)?);
        }
        let face_verts = layout.faces();
        if face_verts.len() != mesh.face_count() {
            return None;
        }
        if (0..mesh.face_count()).any(|face| mesh.face_size(face) != 4) {
            return None;
        }
        let by_points = Buckets::new(
            groups,
            (0..mesh.face_count()).map(|face| {
                let start = mesh.face_offsets[face] as usize;
                let mut key = [0u32; 4];
                for (slot, value) in key.iter_mut().enumerate() {
                    *value = blender_of[mesh.corner_verts[start + slot] as usize];
                }
                key.sort_unstable();
                (key[0], key, face as u32)
            }),
        );
        let mut faces = Vec::with_capacity(face_verts.len());
        let mut corners = Vec::with_capacity(face_verts.len() * 4);
        let mut corner_verts = Vec::with_capacity(face_verts.len() * 4);
        let mut corner_edges = Vec::with_capacity(face_verts.len() * 4);
        for verts in &face_verts {
            let mut key = *verts;
            key.sort_unstable();
            let face = by_points.last(key[0], key)? as usize;
            let start = mesh.face_offsets[face] as usize;
            let mapped: [u32; 4] =
                std::array::from_fn(|slot| blender_of[mesh.corner_verts[start + slot] as usize]);
            let offset = mapped.iter().position(|&point| point == verts[0])?;
            faces.push(face as u32);
            for slot in 0..4 {
                if mapped[(offset + slot) % 4] != verts[slot] {
                    return None;
                }
                corners.push((start + (offset + slot) % 4) as u32);
                corner_verts.push(verts[slot]);
                let [low, high] = ordered([verts[slot], verts[(slot + 1) % 4]]);
                corner_edges.push(lookup.last(low, high)?);
            }
        }
        Some(Order {
            points,
            edges,
            edge_verts,
            faces,
            corners,
            corner_verts,
            corner_edges,
        })
    }
}

struct Order {
    points: Vec<u32>,
    edges: Vec<u32>,
    edge_verts: Vec<[u32; 2]>,
    faces: Vec<u32>,
    corners: Vec<u32>,
    corner_verts: Vec<u32>,
    corner_edges: Vec<u32>,
}

struct Layout<'a> {
    coarse: &'a Coarse,
    resolution: u32,
    patch: u32,
    edge_offset: u32,
    inner_offset: u32,
    face_offsets: Vec<u32>,
}

impl<'a> Layout<'a> {
    fn new(coarse: &'a Coarse, levels: u32) -> Layout<'a> {
        let resolution = (1u32 << levels) + 1;
        let patch = (resolution >> 1) + 1;
        let per_edge = resolution - 2;
        let edge_offset = coarse.points;
        let inner_offset = edge_offset + coarse.edges.len() as u32 * per_edge;
        let mut face_offsets = Vec::with_capacity(coarse.face_offsets.len());
        let mut offset = 0;
        for face in 0..coarse.face_offsets.len() - 1 {
            face_offsets.push(offset);
            let size = coarse.face_len(face) as u32;
            offset += match size {
                4 => per_edge * per_edge,
                _ => 1 + size * (patch - 2) * (patch - 1),
            };
        }
        Layout {
            coarse,
            resolution,
            patch,
            edge_offset,
            inner_offset,
            face_offsets,
        }
    }

    fn per_edge(&self) -> u32 {
        self.resolution - 2
    }

    fn edge_vertex(&self, edge: u32, along: u32) -> u32 {
        self.edge_offset + edge * self.per_edge() + along - 1
    }

    fn inner_vertex(&self, param: Param) -> Option<u32> {
        let coarse = self.coarse;
        let face = param.face as usize;
        let size = coarse.face_len(face);
        let start = self.inner_offset + self.face_offsets[face];
        if size == 4 {
            let full = self.resolution - 1;
            if param.u == 0 || param.v == 0 || param.u == full || param.v == full {
                return None;
            }
            return Some(start + (param.v - 1) * self.per_edge() + param.u - 1);
        }
        let full = self.patch - 1;
        let (mut patch, mut u, mut v) = (param.patch as usize, param.u, param.v);
        if u == full && v == full {
            return Some(start);
        }
        if v == full {
            patch = (patch + size - 1) % size;
            v = u;
            u = full;
        }
        if u == 0 || v == 0 {
            return None;
        }
        let per_patch = full * (full - 1);
        Some(start + 1 + patch as u32 * per_patch + (v - 1) * full + u - 1)
    }

    fn edges(&self) -> Vec<[u32; 2]> {
        let coarse = self.coarse;
        let per_edge = self.per_edge();
        let mut edges = Vec::new();
        for (edge, &[a, b]) in coarse.edges.iter().enumerate() {
            let mut last = a;
            for along in 0..per_edge {
                let next = self.edge_offset + edge as u32 * per_edge + along;
                edges.push([last, next]);
                last = next;
            }
            edges.push([last, b]);
        }
        for face in 0..coarse.face_offsets.len() - 1 {
            match coarse.face_len(face) {
                4 => self.regular_edges(face, &mut edges),
                _ => self.special_edges(face, &mut edges),
            }
        }
        edges
    }

    fn boundary_vertex(&self, edge: u32, corner_vert: u32, step: u32) -> u32 {
        let start = self.edge_offset + edge * self.per_edge();
        match self.coarse.edges[edge as usize][1] == corner_vert {
            true => start + (self.resolution - step - 3),
            false => start + step,
        }
    }

    fn regular_edges(&self, face: usize, edges: &mut Vec<[u32; 2]>) {
        let coarse = self.coarse;
        let resolution = self.resolution;
        let inner = resolution - 2;
        let start = self.inner_offset + self.face_offsets[face];
        row(edges, start, inner);
        for line in 0..inner.saturating_sub(1) {
            let line_start = start + line * inner;
            column(edges, line_start, inner);
            row(edges, line_start + inner, inner);
        }
        for slot in 0..4 {
            let corner = coarse.corner(face, slot);
            let vert = coarse.corner_verts[corner];
            let edge = coarse.corner_edges[corner];
            let (side_start, stride) = regular_side(start, inner, slot);
            for step in 0..inner {
                edges.push([
                    self.boundary_vertex(edge, vert, step),
                    (side_start + stride * step as i64) as u32,
                ]);
            }
        }
    }

    fn special_edges(&self, face: usize, edges: &mut Vec<[u32; 2]>) {
        let coarse = self.coarse;
        let size = coarse.face_len(face);
        let patch = self.patch;
        let inner = patch - 2;
        let per_patch = (patch - 1) * (patch - 2);
        let center = self.inner_offset + self.face_offsets[face];
        let start = center + 1;
        for slot in 0..size as u32 {
            let patch_start = start + slot * per_patch;
            row(edges, patch_start, inner + 1);
            for line in 0..inner.saturating_sub(1) {
                let line_start = patch_start + line * (inner + 1);
                column(edges, line_start, inner + 1);
                row(edges, line_start + inner + 1, inner + 1);
            }
        }
        for slot in 0..size as u32 {
            let next = (slot + 1) % size as u32;
            let first = start + next * per_patch + per_patch - patch + 1;
            for (step, following) in (first..first + inner).enumerate() {
                edges.push([
                    start + slot * per_patch + inner + step as u32 * (inner + 1),
                    following,
                ]);
            }
        }
        if patch >= 3 {
            for slot in 0..size as u32 {
                edges.push([center, start + slot * per_patch + per_patch - 1]);
            }
        }
        for slot in 0..size {
            let previous = (slot + size - 1) % size;
            let corner = coarse.corner(face, slot);
            let vert = coarse.corner_verts[corner];
            let edge = coarse.corner_edges[corner];
            let previous_edge = coarse.corner_edges[coarse.corner(face, previous)];
            let side_start = match patch >= 3 {
                true => start + per_patch * slot as u32,
                false => center,
            };
            for step in 0..patch - 1 {
                edges.push([self.boundary_vertex(edge, vert, step), side_start + step]);
            }
            if patch >= 3 {
                let side_start = start + per_patch * slot as u32;
                for step in 0..patch - 2 {
                    edges.push([
                        self.boundary_vertex(previous_edge, vert, step),
                        side_start + (inner + 1) * step,
                    ]);
                }
            }
        }
    }

    fn faces(&self) -> Vec<[u32; 4]> {
        let coarse = self.coarse;
        let mut faces = Vec::new();
        for face in 0..coarse.face_offsets.len() - 1 {
            match coarse.face_len(face) {
                4 => self.regular_faces(face, &mut faces),
                _ => self.special_faces(face, &mut faces),
            }
        }
        faces
    }

    fn previous_edge_vertex(&self, face: usize, slot: usize) -> u32 {
        let coarse = self.coarse;
        let size = coarse.face_len(face);
        let previous = coarse.corner(face, (slot + size - 1) % size);
        let previous_vert = coarse.corner_verts[previous];
        let previous_edge = coarse.corner_edges[previous];
        let start = self.edge_offset + previous_edge * self.per_edge();
        match coarse.edges[previous_edge as usize][0] == previous_vert {
            true => start + self.per_edge() - 1,
            false => start,
        }
    }

    fn regular_faces(&self, face: usize, faces: &mut Vec<[u32; 4]>) {
        let coarse = self.coarse;
        let resolution = self.resolution;
        let inner = resolution - 2;
        let start = self.inner_offset + self.face_offsets[face];
        for y in 1..resolution.saturating_sub(2) {
            for x in 1..resolution.saturating_sub(2) {
                let v0 = start + (y - 1) * inner + (x - 1);
                faces.push(rotate([v0, v0 + 1, v0 + inner + 1, v0 + inner], 0));
            }
        }
        for slot in 0..4 {
            let corner = coarse.corner(face, slot);
            let vert = coarse.corner_verts[corner];
            let edge = coarse.corner_edges[corner];
            let (side_start, stride) = regular_side(start, inner, slot);
            let mut v0 = vert;
            let mut v3 = self.previous_edge_vertex(face, slot);
            for step in 0..inner {
                let v1 = self.boundary_vertex(edge, vert, step);
                let v2 = (side_start + stride * step as i64) as u32;
                faces.push(rotate([v0, v1, v2, v3], slot));
                v0 = v1;
                v3 = v2;
            }
        }
    }

    fn special_faces(&self, face: usize, faces: &mut Vec<[u32; 4]>) {
        let coarse = self.coarse;
        let size = coarse.face_len(face);
        let patch = self.patch;
        let inner = patch - 2;
        let per_patch = (patch - 1) * (patch - 2);
        let center = self.inner_offset + self.face_offsets[face];
        let start = center + 1;
        for slot in 0..size as u32 {
            let patch_start = start + slot * per_patch;
            for y in 1..inner {
                for x in 1..inner + 1 {
                    let v0 = patch_start + (y - 1) * (inner + 1) + (x - 1);
                    faces.push(rotate([v0, v0 + 1, v0 + inner + 2, v0 + inner + 1], 0));
                }
            }
        }
        for slot in 0..size as u32 {
            let next = (slot + 1) % size as u32;
            let mut current = start + slot * per_patch + inner;
            let mut following = start + next * per_patch + per_patch - patch + 1;
            let mut v0 = current;
            let mut v1 = following;
            current += inner + 1;
            following += 1;
            for _ in 1..inner {
                let v2 = following;
                let v3 = current;
                faces.push(rotate([v0, v1, v2, v3], 3));
                current += inner + 1;
                following += 1;
                v0 = v3;
                v1 = v2;
            }
        }
        if patch >= 3 {
            for slot in 0..size as u32 {
                let previous = (slot + size as u32 - 1) % size as u32;
                let end = start + slot * per_patch + per_patch - 1;
                let previous_end = start + previous * per_patch + per_patch - 1;
                faces.push(rotate([center, previous_end, end - 1, end], 2));
            }
        }
        for slot in 0..size {
            let previous = (slot + size - 1) % size;
            let corner = coarse.corner(face, slot);
            let vert = coarse.corner_verts[corner];
            let edge = coarse.corner_edges[corner];
            let previous_edge = coarse.corner_edges[coarse.corner(face, previous)];
            let patch_start = start + slot as u32 * per_patch;
            let mut v0 = vert;
            let mut v3 = self.previous_edge_vertex(face, slot);
            for step in 0..=inner {
                let v1 = self.boundary_vertex(edge, vert, step);
                let v2 = match inner >= 1 {
                    true => patch_start + step,
                    false => center,
                };
                faces.push(rotate([v0, v1, v2, v3], 0));
                v0 = v1;
                v3 = v2;
            }
            let flip_previous = coarse.edges[previous_edge as usize][1] == vert;
            let mut v0 = patch_start;
            let mut v3 = self.previous_edge_vertex(face, slot);
            for step in 0..inner {
                let v1 = match step + 1 == inner {
                    true => start + previous as u32 * per_patch + inner,
                    false => v0 + inner + 1,
                };
                let v2 = match flip_previous {
                    true => v3 - 1,
                    false => v3 + 1,
                };
                faces.push(rotate([v0, v1, v2, v3], 1));
                v0 = v1;
                v3 = v2;
            }
        }
    }
}

fn regular_side(start: u32, inner: u32, slot: usize) -> (i64, i64) {
    let start = i64::from(start);
    let inner = i64::from(inner);
    match slot {
        0 => (start, 1),
        1 => (start + inner - 1, inner),
        2 => (start + inner * inner - 1, -1),
        _ => (start + inner * (inner - 1), -inner),
    }
}

fn row(edges: &mut Vec<[u32; 2]>, start: u32, count: u32) {
    for step in 0..count.saturating_sub(1) {
        edges.push([start + step, start + step + 1]);
    }
}

fn column(edges: &mut Vec<[u32; 2]>, start: u32, count: u32) {
    for step in 0..count {
        edges.push([start + step, start + step + count]);
    }
}

fn rotate(values: [u32; 4], rotation: usize) -> [u32; 4] {
    [0, 1, 2, 3].map(|slot| values[(slot + 4 - rotation) % 4])
}

struct Buckets<K> {
    offsets: Vec<u32>,
    entries: Vec<(K, u32)>,
}

impl<K: Copy + PartialEq + Default> Buckets<K> {
    fn new(groups: usize, items: impl Iterator<Item = (u32, K, u32)> + Clone) -> Buckets<K> {
        let mut offsets = vec![0u32; groups + 1];
        for (group, _, _) in items.clone() {
            if let Some(count) = offsets.get_mut(group as usize + 1) {
                *count += 1;
            }
        }
        for index in 0..groups {
            offsets[index + 1] += offsets[index];
        }
        let mut cursor = offsets.clone();
        let mut entries = vec![(K::default(), 0u32); offsets[groups] as usize];
        for (group, key, value) in items {
            if let Some(slot) = cursor
                .get_mut(group as usize)
                .filter(|_| (group as usize) < groups)
            {
                entries[*slot as usize] = (key, value);
                *slot += 1;
            }
        }
        Buckets { offsets, entries }
    }

    fn last(&self, group: u32, key: K) -> Option<u32> {
        let group = group as usize;
        let start = *self.offsets.get(group)? as usize;
        let end = *self.offsets.get(group + 1)? as usize;
        self.entries[start..end]
            .iter()
            .rev()
            .find(|entry| entry.0 == key)
            .map(|entry| entry.1)
    }
}
