use glam::Vec2;

use crate::polymesh::Polymesh;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupedIndices {
    pub offsets: Vec<u32>,
    pub indices: Vec<u32>,
}

impl GroupedIndices {
    #[must_use]
    pub fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn get(&self, group: usize) -> &[u32] {
        &self.indices[self.offsets[group] as usize..self.offsets[group + 1] as usize]
    }

    pub fn iter(&self) -> impl Iterator<Item = &[u32]> {
        (0..self.len()).map(|group| self.get(group))
    }

    pub(crate) fn build(
        groups: usize,
        pairs: impl Iterator<Item = (u32, u32)> + Clone,
    ) -> GroupedIndices {
        let mut offsets = vec![0u32; groups + 1];
        for (group, _) in pairs.clone() {
            offsets[group as usize + 1] += 1;
        }
        for index in 1..offsets.len() {
            offsets[index] += offsets[index - 1];
        }
        let mut filled = offsets.clone();
        let mut indices = vec![0; offsets[groups] as usize];
        for (group, member) in pairs {
            let slot = &mut filled[group as usize];
            indices[*slot as usize] = member;
            *slot += 1;
        }
        GroupedIndices { offsets, indices }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Loose,
    Boundary,
    Manifold,
    NonManifold,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Components {
    pub ids: Vec<u32>,
    pub count: usize,
}

impl Components {
    #[must_use]
    pub fn members(&self) -> GroupedIndices {
        GroupedIndices::build(
            self.count,
            self.ids
                .iter()
                .enumerate()
                .map(|(element, &id)| (id, element as u32)),
        )
    }
}

pub(crate) const UV_CONNECT_LIMIT: f32 = 1.0e-4;

struct Unions {
    parents: Vec<u32>,
}

impl Unions {
    fn new(len: usize) -> Unions {
        Unions {
            parents: (0..len as u32).collect(),
        }
    }

    fn root(&mut self, element: u32) -> u32 {
        let mut element = element;
        while self.parents[element as usize] != element {
            let parent = self.parents[element as usize];
            self.parents[element as usize] = self.parents[parent as usize];
            element = parent;
        }
        element
    }

    fn join(&mut self, a: u32, b: u32) {
        let (a, b) = (self.root(a), self.root(b));
        if a != b {
            let (low, high) = (a.min(b), a.max(b));
            self.parents[high as usize] = low;
        }
    }

    fn components(mut self) -> Components {
        let len = self.parents.len();
        let mut ids = vec![u32::MAX; len];
        let mut count = 0;
        let mut roots = vec![u32::MAX; len];
        for element in 0..len as u32 {
            let root = self.root(element);
            if roots[root as usize] == u32::MAX {
                roots[root as usize] = count;
                count += 1;
            }
            ids[element as usize] = roots[root as usize];
        }
        Components {
            ids,
            count: count as usize,
        }
    }
}

impl Polymesh {
    #[must_use]
    pub fn point_edges(&self) -> GroupedIndices {
        GroupedIndices::build(
            self.point_count(),
            self.edges.iter().enumerate().flat_map(|(edge, pair)| {
                let edge = edge as u32;
                let second = (pair[0] != pair[1]).then_some((pair[1], edge));
                std::iter::once((pair[0], edge)).chain(second)
            }),
        )
    }

    #[must_use]
    pub fn point_corners(&self) -> GroupedIndices {
        GroupedIndices::build(
            self.point_count(),
            self.corner_verts
                .iter()
                .enumerate()
                .map(|(corner, &point)| (point, corner as u32)),
        )
    }

    #[must_use]
    pub fn point_faces(&self) -> GroupedIndices {
        let owners = self.corner_faces();
        GroupedIndices::build(
            self.point_count(),
            self.corner_verts
                .iter()
                .zip(&owners)
                .map(|(&point, &face)| (point, face)),
        )
    }

    #[must_use]
    pub fn edge_faces(&self) -> GroupedIndices {
        let owners = self.corner_faces();
        GroupedIndices::build(
            self.edge_count(),
            self.corner_edges
                .iter()
                .zip(&owners)
                .map(|(&edge, &face)| (edge, face)),
        )
    }

    #[must_use]
    pub fn edge_corners(&self) -> GroupedIndices {
        GroupedIndices::build(
            self.edge_count(),
            self.corner_edges
                .iter()
                .enumerate()
                .map(|(corner, &edge)| (edge, corner as u32)),
        )
    }

    #[must_use]
    pub fn face_neighbours(&self) -> GroupedIndices {
        let edge_faces = self.edge_faces();
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        for face in 0..self.face_count() {
            let mut neighbours: Vec<u32> = Vec::new();
            for corner in self.face(face) {
                for &other in edge_faces.get(self.corner_edges[corner] as usize) {
                    if other as usize != face && !neighbours.contains(&other) {
                        neighbours.push(other);
                    }
                }
            }
            pairs.extend(neighbours.into_iter().map(|other| (face as u32, other)));
        }
        GroupedIndices::build(self.face_count(), pairs.into_iter())
    }

    #[must_use]
    pub fn edge_kinds(&self) -> Vec<EdgeKind> {
        let mut users = vec![0u32; self.edge_count()];
        for &edge in &self.corner_edges {
            users[edge as usize] += 1;
        }
        users
            .into_iter()
            .map(|count| match count {
                0 => EdgeKind::Loose,
                1 => EdgeKind::Boundary,
                2 => EdgeKind::Manifold,
                _ => EdgeKind::NonManifold,
            })
            .collect()
    }

    #[must_use]
    pub fn loose_points(&self) -> Vec<bool> {
        let mut loose = vec![true; self.point_count()];
        for edge in &self.edges {
            loose[edge[0] as usize] = false;
            loose[edge[1] as usize] = false;
        }
        for &point in &self.corner_verts {
            loose[point as usize] = false;
        }
        loose
    }

    #[must_use]
    pub fn loose_edges(&self) -> Vec<bool> {
        let mut loose = vec![true; self.edge_count()];
        for &edge in &self.corner_edges {
            loose[edge as usize] = false;
        }
        loose
    }

    #[must_use]
    pub fn is_manifold(&self) -> bool {
        self.edge_kinds()
            .iter()
            .all(|kind| *kind == EdgeKind::Manifold)
            && !self.loose_points().contains(&true)
    }

    #[must_use]
    pub fn point_components(&self) -> Components {
        let mut unions = Unions::new(self.point_count());
        for edge in &self.edges {
            unions.join(edge[0], edge[1]);
        }
        unions.components()
    }

    #[must_use]
    pub fn face_components(&self) -> Components {
        let mut unions = Unions::new(self.face_count());
        let mut first_face = vec![u32::MAX; self.edge_count()];
        let owners = self.corner_faces();
        for (corner, &edge) in self.corner_edges.iter().enumerate() {
            let face = owners[corner];
            match first_face[edge as usize] {
                u32::MAX => first_face[edge as usize] = face,
                other => unions.join(other, face),
            }
        }
        unions.components()
    }

    #[must_use]
    pub fn uv_islands(&self, uv_map: &str, delimit_seams: bool) -> Option<Components> {
        let uvs = self.uv_map(uv_map)?;
        let owners = self.corner_faces();
        let edge_corners = self.edge_corners();
        let mut unions = Unions::new(self.face_count());
        let close = |a: Vec2, b: Vec2| (a - b).abs().max_element() <= UV_CONNECT_LIMIT;
        for edge in 0..self.edge_count() {
            if delimit_seams && self.is_uv_seam(edge) {
                continue;
            }
            let corners = edge_corners.get(edge);
            for (slot, &first) in corners.iter().enumerate() {
                for &second in &corners[slot + 1..] {
                    let (first, second) = (first as usize, second as usize);
                    let (face_a, face_b) = (owners[first] as usize, owners[second] as usize);
                    let first_next = self.next_corner(face_a, first);
                    let second_next = self.next_corner(face_b, second);
                    let matched = match self.corner_verts[first] == self.corner_verts[second] {
                        true => {
                            close(uvs[first], uvs[second])
                                && close(uvs[first_next], uvs[second_next])
                        }
                        false => {
                            close(uvs[first], uvs[second_next])
                                && close(uvs[first_next], uvs[second])
                        }
                    };
                    if matched {
                        unions.join(face_a as u32, face_b as u32);
                    }
                }
            }
        }
        Some(unions.components())
    }
}
