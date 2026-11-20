use glam::Mat4;

use crate::domain::Domain;
use crate::names::CUSTOM_NORMAL;
use crate::polymesh::Polymesh;
use crate::values::Values;

impl Polymesh {
    pub fn retain_faces(&mut self, keep: &[bool]) {
        let kept: Vec<(u32, Vec<u32>)> = (0..self.face_count())
            .filter(|&face| keep.get(face).copied().unwrap_or(true))
            .map(|face| {
                let range = self.face(face);
                (
                    face as u32,
                    (range.start as u32..range.end as u32).collect(),
                )
            })
            .collect();
        if kept.len() == self.face_count() {
            return;
        }
        self.rebuild_faces(
            kept.iter()
                .map(|(face, corners)| (*face, corners.as_slice())),
        );
    }

    pub fn retain_edges(&mut self, keep: &[bool]) {
        let kept = |edge: usize| keep.get(edge).copied().unwrap_or(true);
        if (0..self.edge_count()).all(kept) {
            return;
        }
        let faces: Vec<bool> = (0..self.face_count())
            .map(|face| {
                self.face(face)
                    .all(|corner| kept(self.corner_edges[corner] as usize))
            })
            .collect();
        self.retain_faces(&faces);
        let mut map = vec![u32::MAX; self.edge_count()];
        let mut survivors: Vec<u32> = Vec::new();
        for (edge, slot) in map.iter_mut().enumerate() {
            if kept(edge) {
                *slot = survivors.len() as u32;
                survivors.push(edge as u32);
            }
        }
        self.gather(Domain::Edge, &survivors);
        for edge in &mut self.corner_edges {
            *edge = map[*edge as usize];
        }
    }

    pub fn retain_points(&mut self, keep: &[bool]) {
        let kept = |point: usize| keep.get(point).copied().unwrap_or(true);
        if (0..self.point_count()).all(kept) {
            return;
        }
        let edges: Vec<bool> = self
            .edges
            .iter()
            .map(|edge| kept(edge[0] as usize) && kept(edge[1] as usize))
            .collect();
        let faces: Vec<bool> = (0..self.face_count())
            .map(|face| {
                self.face_verts(face)
                    .iter()
                    .all(|&point| kept(point as usize))
            })
            .collect();
        self.retain_faces(&faces);
        self.retain_edges(&edges);
        let mut map = vec![u32::MAX; self.point_count()];
        let mut survivors: Vec<u32> = Vec::new();
        for (point, slot) in map.iter_mut().enumerate() {
            if kept(point) {
                *slot = survivors.len() as u32;
                survivors.push(point as u32);
            }
        }
        self.gather(Domain::Point, &survivors);
        for point in &mut self.corner_verts {
            *point = map[*point as usize];
        }
        for edge in &mut self.edges {
            *edge = edge.map(|point| map[point as usize]);
        }
    }

    pub fn remove_loose(&mut self) -> bool {
        let loose_edges = self.loose_edges();
        let removed_edges = loose_edges.contains(&true);
        if removed_edges {
            let keep: Vec<bool> = loose_edges.iter().map(|loose| !loose).collect();
            self.retain_edges(&keep);
        }
        let loose_points = self.loose_points();
        let removed_points = loose_points.contains(&true);
        if removed_points {
            let keep: Vec<bool> = loose_points.iter().map(|loose| !loose).collect();
            self.retain_points(&keep);
        }
        removed_edges || removed_points
    }

    #[must_use]
    pub fn extract(&self, faces: &[u32]) -> Polymesh {
        let mut part = self.clone();
        let chosen: Vec<(u32, Vec<u32>)> = faces
            .iter()
            .filter(|&&face| (face as usize) < self.face_count())
            .map(|&face| {
                let range = self.face(face as usize);
                (face, (range.start as u32..range.end as u32).collect())
            })
            .collect();
        part.rebuild_faces(
            chosen
                .iter()
                .map(|(face, corners)| (*face, corners.as_slice())),
        );
        part.remove_loose();
        part
    }

    #[must_use]
    pub fn join(parts: &[(&Polymesh, Mat4)]) -> Polymesh {
        let mut joined = Polymesh::new();
        for (part, matrix) in parts {
            let mut placed = (*part).clone();
            placed.transform(matrix);
            joined.append(&placed);
        }
        joined
    }

    pub fn append(&mut self, other: &Polymesh) {
        let sizes = self.sizes();
        let other_sizes = other.sizes();
        let mut other_attributes = other.attributes.clone();
        for attribute in other_attributes.iter_mut() {
            if let Some(mine) = self.attributes.get(&attribute.name)
                && mine.domain != attribute.domain
            {
                attribute.values =
                    other.adapt_domain(&attribute.values, attribute.domain, mine.domain);
                attribute.domain = mine.domain;
            }
        }
        let mine_custom = self.attributes.contains(CUSTOM_NORMAL);
        let theirs_custom = other_attributes.contains(CUSTOM_NORMAL);
        if mine_custom && !theirs_custom && other_sizes.corners > 0 {
            other_attributes.insert(crate::attributes::Attribute::new(
                CUSTOM_NORMAL,
                Domain::Corner,
                Values::Vec3(other.corner_normals()),
            ));
        }
        if theirs_custom && !mine_custom && sizes.corners > 0 {
            let normals = self.corner_normals();
            self.custom_normals_mut().copy_from_slice(&normals);
        }
        self.attributes
            .extend_from(&other_attributes, sizes, other_sizes);
        self.uv_maps.union(&other.uv_maps);
        self.color_attributes.union(&other.color_attributes);
        self.vertex_groups
            .extend_from(&other.vertex_groups, sizes.points, other_sizes.points);
        self.shape_keys
            .extend_from(&other.shape_keys, &self.positions, &other.positions);

        let points = sizes.points as u32;
        let edges = sizes.edges as u32;
        let corners = sizes.corners as u32;
        self.positions.extend_from_slice(&other.positions);
        self.edges.extend(
            other
                .edges
                .iter()
                .map(|edge| edge.map(|point| point + points)),
        );
        self.face_offsets.extend(
            other
                .face_offsets
                .iter()
                .skip(1)
                .map(|offset| offset + corners),
        );
        self.corner_verts
            .extend(other.corner_verts.iter().map(|point| point + points));
        self.corner_edges
            .extend(other.corner_edges.iter().map(|edge| edge + edges));
    }
}
