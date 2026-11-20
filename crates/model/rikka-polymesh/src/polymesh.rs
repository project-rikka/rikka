use std::ops::Range;

use glam::{Vec2, Vec3, Vec4};

use crate::attributes::{Attribute, Attributes};
use crate::domain::{Domain, DomainSizes};
use crate::layers::Layers;
use crate::names::uv_pin_name;
use crate::parallel;
use crate::shape_keys::ShapeKeys;
use crate::values::Values;
use crate::vertex_groups::VertexGroups;

#[derive(Clone, Debug, PartialEq)]
pub struct Polymesh {
    pub positions: Vec<Vec3>,
    pub edges: Vec<[u32; 2]>,
    pub face_offsets: Vec<u32>,
    pub corner_verts: Vec<u32>,
    pub corner_edges: Vec<u32>,
    pub attributes: Attributes,
    pub uv_maps: Layers,
    pub color_attributes: Layers,
    pub vertex_groups: VertexGroups,
    pub shape_keys: ShapeKeys,
}

impl Default for Polymesh {
    fn default() -> Polymesh {
        Polymesh {
            positions: Vec::new(),
            edges: Vec::new(),
            face_offsets: vec![0],
            corner_verts: Vec::new(),
            corner_edges: Vec::new(),
            attributes: Attributes::new(),
            uv_maps: Layers::default(),
            color_attributes: Layers::default(),
            vertex_groups: VertexGroups::default(),
            shape_keys: ShapeKeys::default(),
        }
    }
}

impl Polymesh {
    #[must_use]
    pub fn new() -> Polymesh {
        Polymesh::default()
    }

    pub fn from_polygons<P: AsRef<[u32]>>(
        positions: Vec<Vec3>,
        polygons: impl IntoIterator<Item = P>,
    ) -> Polymesh {
        let mut mesh = Polymesh {
            positions,
            ..Polymesh::default()
        };
        for polygon in polygons {
            mesh.corner_verts.extend_from_slice(polygon.as_ref());
            mesh.face_offsets.push(mesh.corner_verts.len() as u32);
        }
        mesh.ensure_edges();
        mesh
    }

    #[must_use]
    pub fn point_count(&self) -> usize {
        self.positions.len()
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    #[must_use]
    pub fn face_count(&self) -> usize {
        self.face_offsets.len().saturating_sub(1)
    }

    #[must_use]
    pub fn corner_count(&self) -> usize {
        self.corner_verts.len()
    }

    #[must_use]
    pub fn domain_len(&self, domain: Domain) -> usize {
        match domain {
            Domain::Point => self.point_count(),
            Domain::Edge => self.edge_count(),
            Domain::Face => self.face_count(),
            Domain::Corner => self.corner_count(),
        }
    }

    #[must_use]
    pub fn sizes(&self) -> DomainSizes {
        DomainSizes {
            points: self.point_count(),
            edges: self.edge_count(),
            faces: self.face_count(),
            corners: self.corner_count(),
        }
    }

    #[must_use]
    pub fn face(&self, face: usize) -> Range<usize> {
        self.face_offsets[face] as usize..self.face_offsets[face + 1] as usize
    }

    #[must_use]
    pub fn face_verts(&self, face: usize) -> &[u32] {
        &self.corner_verts[self.face(face)]
    }

    #[must_use]
    pub fn face_size(&self, face: usize) -> usize {
        (self.face_offsets[face + 1] - self.face_offsets[face]) as usize
    }

    #[must_use]
    pub fn next_corner(&self, face: usize, corner: usize) -> usize {
        let range = self.face(face);
        match corner + 1 == range.end {
            true => range.start,
            false => corner + 1,
        }
    }

    #[must_use]
    pub fn previous_corner(&self, face: usize, corner: usize) -> usize {
        let range = self.face(face);
        match corner == range.start {
            true => range.end - 1,
            false => corner - 1,
        }
    }

    #[must_use]
    pub fn corner_faces(&self) -> Vec<u32> {
        let mut owners = vec![0; self.corner_count()];
        for face in 0..self.face_count() {
            owners[self.face(face)].fill(face as u32);
        }
        owners
    }

    pub fn resize(&mut self, domain: Domain, len: usize) {
        match domain {
            Domain::Point => {
                self.positions.resize(len, Vec3::ZERO);
                self.vertex_groups.resize(len);
                self.shape_keys.resize(len);
            }
            Domain::Edge => self.edges.resize(len, [0, 0]),
            Domain::Face => {}
            Domain::Corner => {
                self.corner_verts.resize(len, 0);
                self.corner_edges.resize(len, 0);
            }
        }
        self.attributes.resize(domain, len);
    }

    pub fn gather(&mut self, domain: Domain, indices: &[u32]) {
        let pick = |list: &[u32]| -> Vec<u32> { parallel::gather(list, indices) };
        match domain {
            Domain::Point => {
                self.positions = parallel::gather(&self.positions, indices);
                self.vertex_groups.gather(indices);
                self.shape_keys.gather(indices);
            }
            Domain::Edge => {
                self.edges = parallel::gather(&self.edges, indices);
            }
            Domain::Face => {}
            Domain::Corner => {
                self.corner_verts = pick(&self.corner_verts);
                self.corner_edges = pick(&self.corner_edges);
            }
        }
        self.attributes.gather(domain, indices);
    }

    pub fn copy(&mut self, domain: Domain, from: usize, to: usize) {
        match domain {
            Domain::Point => {
                self.positions[to] = self.positions[from];
                self.vertex_groups.copy(from, to);
                self.shape_keys.copy(from, to);
            }
            Domain::Edge => self.edges[to] = self.edges[from],
            Domain::Face => {}
            Domain::Corner => {
                self.corner_verts[to] = self.corner_verts[from];
                self.corner_edges[to] = self.corner_edges[from];
            }
        }
        self.attributes.copy(domain, from, to);
    }

    pub fn interpolate(&mut self, domain: Domain, sources: &[(u32, f32)], to: usize) {
        match domain {
            Domain::Point => {
                self.positions[to] = mix_positions(&self.positions, sources);
                self.vertex_groups.interpolate(sources, to);
                self.shape_keys.interpolate(sources, to);
            }
            Domain::Edge => {
                if let Some(strongest) = strongest(sources) {
                    self.edges[to] = self.edges[strongest];
                }
            }
            Domain::Face => {}
            Domain::Corner => {
                if let Some(strongest) = strongest(sources) {
                    self.corner_verts[to] = self.corner_verts[strongest];
                    self.corner_edges[to] = self.corner_edges[strongest];
                }
            }
        }
        self.attributes.interpolate(domain, sources, to);
    }

    pub fn push_copy(&mut self, domain: Domain, from: usize) -> usize {
        let index = self.domain_len(domain);
        match domain {
            Domain::Point => {
                self.positions.push(self.positions[from]);
                self.vertex_groups.push_copy(from);
                self.shape_keys.push_copy(from);
            }
            Domain::Edge => self.edges.push(self.edges[from]),
            Domain::Face => {}
            Domain::Corner => {
                self.corner_verts.push(self.corner_verts[from]);
                self.corner_edges.push(self.corner_edges[from]);
            }
        }
        self.attributes.push_copy(domain, from);
        index
    }

    pub fn push_interpolated(&mut self, domain: Domain, sources: &[(u32, f32)]) -> usize {
        let index = self.domain_len(domain);
        match domain {
            Domain::Point => {
                self.positions.push(mix_positions(&self.positions, sources));
                self.vertex_groups.push_interpolated(sources);
                self.shape_keys.push_interpolated(sources);
            }
            Domain::Edge => {
                let pair = strongest(sources).map_or([0, 0], |strongest| self.edges[strongest]);
                self.edges.push(pair);
            }
            Domain::Face => {}
            Domain::Corner => {
                let source = strongest(sources);
                self.corner_verts
                    .push(source.map_or(0, |source| self.corner_verts[source]));
                self.corner_edges
                    .push(source.map_or(0, |source| self.corner_edges[source]));
            }
        }
        self.attributes.push_interpolated(domain, sources);
        index
    }

    pub fn rebuild_faces<'a>(&mut self, faces: impl IntoIterator<Item = (u32, &'a [u32])>) {
        let mut offsets = vec![0u32];
        let mut corners: Vec<u32> = Vec::new();
        let mut owners: Vec<u32> = Vec::new();
        for (face, face_corners) in faces {
            corners.extend_from_slice(face_corners);
            offsets.push(corners.len() as u32);
            owners.push(face);
        }
        self.gather(Domain::Face, &owners);
        self.gather(Domain::Corner, &corners);
        self.face_offsets = offsets;
        self.ensure_edges();
    }

    #[must_use]
    pub fn uv_map(&self, name: &str) -> Option<&[Vec2]> {
        self.uv_maps
            .contains(name)
            .then(|| self.attributes.values(name, Domain::Corner))
            .flatten()
    }

    pub fn uv_map_mut(&mut self, name: &str) -> Option<&mut [Vec2]> {
        match self.uv_maps.contains(name) {
            true => self.attributes.values_mut(name, Domain::Corner),
            false => None,
        }
    }

    #[must_use]
    pub fn active_uv_map(&self) -> Option<&[Vec2]> {
        self.uv_map(self.uv_maps.active_name()?)
    }

    #[must_use]
    pub fn render_uv_map(&self) -> Option<&[Vec2]> {
        self.uv_map(self.uv_maps.render_name()?)
    }

    pub fn add_uv_map(&mut self, name: impl Into<String>, values: Vec<Vec2>) {
        let name = name.into();
        self.attributes.insert(Attribute::new(
            name.clone(),
            Domain::Corner,
            Values::Vec2(values),
        ));
        self.uv_maps.push(name);
    }

    pub fn remove_uv_map(&mut self, name: &str) -> bool {
        if !self.uv_maps.remove(name) {
            return false;
        }
        self.attributes.remove(name);
        self.attributes.remove(&uv_pin_name(name));
        true
    }

    #[must_use]
    pub fn uv_pins(&self, uv_map: &str) -> Option<&[bool]> {
        self.attributes.values(&uv_pin_name(uv_map), Domain::Corner)
    }

    pub fn uv_pins_mut(&mut self, uv_map: &str) -> &mut [bool] {
        let corners = self.corner_count();
        self.attributes
            .ensure(&uv_pin_name(uv_map), Domain::Corner, corners)
    }

    #[must_use]
    pub fn color_attribute(&self, name: &str) -> Option<&Attribute> {
        self.color_attributes
            .contains(name)
            .then(|| self.attributes.get(name))
            .flatten()
            .filter(|attribute| {
                matches!(attribute.domain, Domain::Point | Domain::Corner)
                    && matches!(attribute.values, Values::Color(_))
            })
    }

    #[must_use]
    pub fn active_color_attribute(&self) -> Option<&Attribute> {
        self.color_attribute(self.color_attributes.active_name()?)
    }

    #[must_use]
    pub fn render_color_attribute(&self) -> Option<&Attribute> {
        self.color_attribute(self.color_attributes.render_name()?)
    }

    pub fn add_color_attribute(
        &mut self,
        name: impl Into<String>,
        domain: Domain,
        values: Vec<Vec4>,
    ) {
        let name = name.into();
        self.attributes
            .insert(Attribute::new(name.clone(), domain, Values::Color(values)));
        self.color_attributes.push(name);
    }

    pub fn remove_color_attribute(&mut self, name: &str) -> bool {
        if !self.color_attributes.remove(name) {
            return false;
        }
        self.attributes.remove(name);
        true
    }

    pub fn remove_attribute(&mut self, name: &str) -> Option<Attribute> {
        let removed = self.attributes.remove(name)?;
        if self.uv_maps.remove(name) {
            self.attributes.remove(&uv_pin_name(name));
        }
        self.color_attributes.remove(name);
        Some(removed)
    }

    pub fn rename_attribute(&mut self, from: &str, to: &str) -> bool {
        if !self.attributes.rename(from, to) {
            return false;
        }
        if self.uv_maps.contains(from) {
            self.uv_maps.rename(from, to);
            self.attributes.rename(&uv_pin_name(from), &uv_pin_name(to));
        }
        self.color_attributes.rename(from, to);
        true
    }
}

fn mix_positions(positions: &[Vec3], sources: &[(u32, f32)]) -> Vec3 {
    sources
        .iter()
        .map(|&(index, weight)| positions[index as usize] * weight)
        .sum()
}

fn strongest(sources: &[(u32, f32)]) -> Option<usize> {
    sources
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|&(index, _)| index as usize)
}
