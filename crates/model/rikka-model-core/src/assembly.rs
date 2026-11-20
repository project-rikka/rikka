use glam::{Vec2, Vec3, Vec4};
use rustc_hash::FxHashMap;

use crate::mesh::{Mesh, Submesh, Vertices};
use crate::polygon;
use crate::polygons::Polygons;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corner {
    pub position: Vec3,
    pub normal: Vec3,
    pub tangent: Option<Vec4>,
    pub uv: Option<Vec2>,
    pub color: Option<[u8; 4]>,
}

impl Corner {
    #[must_use]
    pub fn new(position: Vec3, normal: Vec3) -> Self {
        Self {
            position,
            normal,
            tangent: None,
            uv: None,
            color: None,
        }
    }

    #[must_use]
    pub fn color_from(color: Vec4) -> [u8; 4] {
        color
            .to_array()
            .map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

#[derive(PartialEq, Eq, Hash)]
struct Key {
    control: u32,
    position: [u32; 3],
    normal: [u32; 3],
    tangent: [u32; 4],
    uv: [u32; 2],
    color: Option<[u8; 4]>,
}

fn bits<const N: usize>(values: [f32; N]) -> [u32; N] {
    values.map(|value| match value.is_nan() {
        true => 0,
        false => (value + 0.0).to_bits(),
    })
}

#[derive(Debug)]
pub struct Assembled {
    pub mesh: Mesh,
    pub slots: Vec<u32>,
    pub controls: Vec<u32>,
}

impl Assembled {
    #[must_use]
    pub fn vertices_of_controls(&self, controls: usize) -> Vec<Vec<u32>> {
        let mut vertices = vec![Vec::new(); controls];
        for (vertex, &control) in self.controls.iter().enumerate() {
            if let Some(of_control) = vertices.get_mut(control as usize) {
                of_control.push(vertex as u32);
            }
        }
        vertices
    }
}

#[derive(Default)]
pub struct MeshAssembler {
    vertices: Vertices,
    welded: FxHashMap<Key, u32>,
    controls: Vec<u32>,
    triangles: Vec<[u32; 3]>,
    slots: Vec<u32>,
    tangents: bool,
    uvs: bool,
    colors: bool,
    outline: Vec<Vec3>,
    polygon_offsets: Vec<u32>,
    polygon_corners: Vec<u32>,
    polygon_slots: Vec<u32>,
    bare_triangles: bool,
}

impl MeshAssembler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.controls.len()
    }

    pub fn corner(&mut self, control: u32, corner: Corner) -> u32 {
        let key = Key {
            control,
            position: bits(corner.position.to_array()),
            normal: bits(corner.normal.to_array()),
            tangent: bits(corner.tangent.unwrap_or(Vec4::ZERO).to_array()),
            uv: bits(corner.uv.unwrap_or(Vec2::ZERO).to_array()),
            color: corner.color,
        };
        if let Some(&vertex) = self.welded.get(&key) {
            return vertex;
        }
        let vertex = self.controls.len() as u32;
        self.vertices.positions.push(corner.position);
        self.vertices.normals.push(corner.normal);
        self.vertices
            .tangents
            .push(corner.tangent.unwrap_or(Vec4::X));
        self.vertices.uvs.push(corner.uv.unwrap_or(Vec2::ZERO));
        self.vertices.colors.push(corner.color.unwrap_or([255; 4]));
        self.tangents |= corner.tangent.is_some();
        self.uvs |= corner.uv.is_some();
        self.colors |= corner.color.is_some();
        self.controls.push(control);
        self.welded.insert(key, vertex);
        vertex
    }

    pub fn triangle(&mut self, vertices: [u32; 3], slot: u32) {
        self.bare_triangles = true;
        self.push_triangle(vertices, slot);
    }

    pub fn polygon(&mut self, vertices: &[u32], slot: u32) {
        if vertices.len() < 3 {
            return;
        }
        self.record_polygon(vertices, slot);
        let mut outline = std::mem::take(&mut self.outline);
        outline.clear();
        outline.extend(
            vertices
                .iter()
                .map(|&vertex| self.vertices.positions[vertex as usize]),
        );
        for [a, b, c] in polygon::triangulate(&outline) {
            self.push_triangle(
                [
                    vertices[a as usize],
                    vertices[b as usize],
                    vertices[c as usize],
                ],
                slot,
            );
        }
        self.outline = outline;
    }

    fn record_polygon(&mut self, vertices: &[u32], slot: u32) {
        if self.polygon_offsets.is_empty() {
            self.polygon_offsets.push(0);
        }
        self.polygon_corners.extend_from_slice(vertices);
        self.polygon_offsets.push(self.polygon_corners.len() as u32);
        self.polygon_slots.push(slot);
    }

    fn push_triangle(&mut self, vertices: [u32; 3], slot: u32) {
        self.triangles.push(vertices);
        self.slots.push(slot);
    }

    #[must_use]
    pub fn finish(mut self, name: &str) -> Assembled {
        let mut order: Vec<u32> = (0..self.triangles.len() as u32).collect();
        order.sort_by_key(|&triangle| self.slots[triangle as usize]);

        let mut indices = Vec::with_capacity(self.triangles.len() * 3);
        let mut submeshes: Vec<Submesh> = Vec::new();
        let mut slots: Vec<u32> = Vec::new();
        for triangle in order {
            let slot = self.slots[triangle as usize];
            match submeshes.last_mut() {
                Some(submesh) if slots.last() == Some(&slot) => submesh.index_count += 3,
                _ => {
                    slots.push(slot);
                    submeshes.push(Submesh {
                        first_index: indices.len() as u32,
                        index_count: 3,
                        ..Submesh::default()
                    });
                }
            }
            indices.extend_from_slice(&self.triangles[triangle as usize]);
        }

        if !self.tangents {
            self.vertices.tangents.clear();
        }
        if !self.uvs {
            self.vertices.uvs.clear();
        }
        if !self.colors {
            self.vertices.colors.clear();
        }
        let polygons = (!self.bare_triangles && !self.polygon_slots.is_empty()).then(|| Polygons {
            vertex_points: self.controls.clone(),
            offsets: self.polygon_offsets,
            corner_vertices: self.polygon_corners,
            submeshes: self
                .polygon_slots
                .iter()
                .map(|slot| {
                    slots
                        .iter()
                        .position(|submesh_slot| submesh_slot == slot)
                        .map_or(0, |submesh| submesh as u32)
                })
                .collect(),
        });
        Assembled {
            mesh: Mesh {
                name: name.to_owned(),
                vertices: self.vertices,
                indices,
                submeshes,
                morph_targets: Vec::new(),
                polygons,
            },
            slots,
            controls: self.controls,
        }
    }
}

impl std::fmt::Debug for MeshAssembler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshAssembler")
            .field("vertices", &self.controls.len())
            .field("triangles", &self.triangles.len())
            .finish_non_exhaustive()
    }
}
