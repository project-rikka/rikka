use glam::{Vec2, Vec3, Vec4};
use rikka_model_core::{Corner, MaterialId, Mesh, MeshAssembler, MorphTarget, Polygons};

use crate::attributes::Attributes;
use crate::domain::Domain;
use crate::names::{CUSTOM_NORMAL, SHARP_EDGE, SHARP_FACE};
use crate::normals::Shading;
use crate::polymesh::Polymesh;
use crate::tangent::{TangentFaces, tangents};

const MOVED: f32 = 1.0e-6;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ColorSource {
    #[default]
    None,
    Render,
    Named(String),
}

#[derive(Clone, Debug, Default)]
#[must_use]
pub struct ToMesh {
    name: String,
    shading: Shading,
    uv_map: Option<String>,
    colors: ColorSource,
    tangents: bool,
    joints: Vec<Option<u16>>,
    materials: Vec<Option<MaterialId>>,
}

impl ToMesh {
    pub fn new(name: impl Into<String>) -> ToMesh {
        ToMesh {
            name: name.into(),
            tangents: true,
            ..ToMesh::default()
        }
    }

    pub fn shading(mut self, shading: Shading) -> ToMesh {
        self.shading = shading;
        self
    }

    pub fn uv_map(mut self, name: impl Into<String>) -> ToMesh {
        self.uv_map = Some(name.into());
        self
    }

    pub fn colors(mut self, colors: ColorSource) -> ToMesh {
        self.colors = colors;
        self
    }

    pub fn tangents(mut self, tangents: bool) -> ToMesh {
        self.tangents = tangents;
        self
    }

    pub fn joints(mut self, joints: impl Into<Vec<Option<u16>>>) -> ToMesh {
        self.joints = joints.into();
        self
    }

    pub fn materials(mut self, materials: impl Into<Vec<Option<MaterialId>>>) -> ToMesh {
        self.materials = materials.into();
        self
    }
}

#[derive(Clone, Debug)]
pub struct ConvertedMesh {
    pub mesh: Mesh,
    pub slots: Vec<u32>,
    pub vertex_points: Vec<u32>,
    pub triangle_faces: Vec<u32>,
    pub shape_keys: Vec<u32>,
}

impl Polymesh {
    #[must_use]
    pub fn to_mesh(&self, options: &ToMesh) -> ConvertedMesh {
        let normals = self.shade(options.shading);
        let uv_name = options
            .uv_map
            .as_deref()
            .filter(|name| self.uv_maps.contains(name))
            .or(self.uv_maps.render_name());
        let uvs = uv_name.and_then(|name| self.uv_map(name));
        let colors = self.corner_colors(&options.colors);
        let triangulation = self.triangulate();
        let tangents = match (uvs, options.tangents) {
            (Some(uvs), true) => {
                Some(self.corner_tangents(&triangulation.triangles, &normals, uvs))
            }
            _ => None,
        };

        let mut assembler = MeshAssembler::new();
        let mut sources: Vec<u32> = Vec::new();
        let mut corner_vertices = vec![0u32; self.corner_count()];
        for corner in 0..self.corner_count() {
            let point = self.corner_verts[corner];
            let vertex = assembler.corner(
                point,
                Corner {
                    tangent: tangents.as_ref().map(|tangents| tangents[corner]),
                    uv: uvs.map(|uvs| Vec2::new(uvs[corner].x, 1.0 - uvs[corner].y)),
                    color: colors
                        .as_ref()
                        .map(|colors| Corner::color_from(colors[corner])),
                    ..Corner::new(self.positions[point as usize], normals[corner])
                },
            );
            if vertex as usize == sources.len() {
                sources.push(corner as u32);
            }
            corner_vertices[corner] = vertex;
        }

        let materials = self.material_indices();
        let slot_of = |face: usize| {
            materials
                .and_then(|indices| indices.get(face).copied())
                .unwrap_or(0)
                .max(0) as u32
        };
        for (triangle, &face) in triangulation.triangles.iter().zip(&triangulation.faces) {
            assembler.triangle(
                triangle.map(|corner| corner_vertices[corner as usize]),
                slot_of(face as usize),
            );
        }
        let assembled = assembler.finish(&options.name);
        let mut mesh = assembled.mesh;
        let slots = assembled.slots;
        for (submesh, &slot) in mesh.submeshes.iter_mut().zip(&slots) {
            submesh.name = format!("{}.{slot}", options.name);
            let material = match options.materials.len() {
                0 => None,
                count => options.materials[(slot as usize).min(count - 1)],
            };
            submesh.material = material;
        }
        let triangle_faces = self.sorted_triangle_faces(&triangulation.faces, &slot_of);

        self.skin(&mut mesh, &sources, &options.joints);
        let shape_keys = self.morph_targets(&mut mesh, &sources, &normals, options.shading);

        let vertex_points: Vec<u32> = sources
            .iter()
            .map(|&corner| self.corner_verts[corner as usize])
            .collect();
        mesh.polygons = Some(Polygons {
            vertex_points: vertex_points.clone(),
            offsets: self.face_offsets.clone(),
            corner_vertices,
            submeshes: (0..self.face_count())
                .map(|face| {
                    let slot = slot_of(face);
                    slots
                        .iter()
                        .position(|&submesh_slot| submesh_slot == slot)
                        .map_or(0, |submesh| submesh as u32)
                })
                .collect(),
        });
        ConvertedMesh {
            mesh,
            slots,
            vertex_points,
            triangle_faces,
            shape_keys,
        }
    }

    fn corner_colors(&self, source: &ColorSource) -> Option<Vec<Vec4>> {
        let attribute = match source {
            ColorSource::None => None,
            ColorSource::Render => self.render_color_attribute(),
            ColorSource::Named(name) => self.color_attribute(name),
        }?;
        let colors = attribute.typed::<Vec4>()?;
        Some(match attribute.domain {
            Domain::Corner => colors.to_vec(),
            _ => self
                .corner_verts
                .iter()
                .map(|&point| colors[point as usize])
                .collect(),
        })
    }

    fn corner_tangents(&self, triangles: &[[u32; 3]], normals: &[Vec3], uvs: &[Vec2]) -> Vec<Vec4> {
        let mut offsets = vec![0u32];
        let mut corners: Vec<u32> = Vec::with_capacity(self.corner_count());
        let mut triangle = 0;
        for face in 0..self.face_count() {
            let range = self.face(face);
            let size = range.len();
            match size {
                4 => {
                    corners.extend(range.start as u32..range.end as u32);
                    offsets.push(corners.len() as u32);
                    triangle += 2;
                }
                _ => {
                    for _ in 0..size - 2 {
                        corners.extend_from_slice(&triangles[triangle]);
                        offsets.push(corners.len() as u32);
                        triangle += 1;
                    }
                }
            }
        }
        let positions: Vec<Vec3> = self
            .corner_verts
            .iter()
            .map(|&point| self.positions[point as usize])
            .collect();
        let mut tangents = tangents(
            &TangentFaces {
                offsets: &offsets,
                corners: &corners,
                positions: &positions,
                normals,
                uvs,
            },
            self.corner_count(),
        );
        for tangent in &mut tangents {
            tangent.w = -tangent.w;
        }
        tangents
    }

    fn sorted_triangle_faces(&self, faces: &[u32], slot_of: &dyn Fn(usize) -> u32) -> Vec<u32> {
        let mut order: Vec<u32> = (0..faces.len() as u32).collect();
        order.sort_by_key(|&triangle| slot_of(faces[triangle as usize] as usize));
        order
            .into_iter()
            .map(|triangle| faces[triangle as usize])
            .collect()
    }

    fn skin(&self, mesh: &mut Mesh, sources: &[u32], joints: &[Option<u16>]) {
        let groups = &self.vertex_groups;
        if joints.iter().all(Option::is_none) || !groups.has_weights() {
            return;
        }
        let vertices = &mut mesh.vertices;
        let mut influences: Vec<(u16, f32)> = Vec::new();
        for (vertex, &corner) in sources.iter().enumerate() {
            let point = self.corner_verts[corner as usize] as usize;
            influences.clear();
            for influence in &groups.weights[point] {
                if influence.weight <= 0.0 {
                    continue;
                }
                let Some(joint) = joints.get(influence.group as usize).copied().flatten() else {
                    continue;
                };
                match influences
                    .iter_mut()
                    .find(|(existing, _)| *existing == joint)
                {
                    Some((_, weight)) => *weight += influence.weight,
                    None => influences.push((joint, influence.weight)),
                }
            }
            influences.sort_by(|a, b| b.1.total_cmp(&a.1));
            influences.truncate(rikka_model_core::INFLUENCES);
            let total: f32 = influences.iter().map(|(_, weight)| weight).sum();
            if total > 0.0 {
                for (_, weight) in &mut influences {
                    *weight /= total;
                }
            }
            vertices.set_influences(vertex, &influences);
        }
        vertices.joints.resize(vertices.len(), [0; 4]);
        vertices.weights.resize(vertices.len(), Vec4::ZERO);
    }

    fn morph_targets(
        &self,
        mesh: &mut Mesh,
        sources: &[u32],
        normals: &[Vec3],
        shading: Shading,
    ) -> Vec<u32> {
        let keys = &self.shape_keys.keys;
        if keys.len() < 2 {
            return Vec::new();
        }
        let mut morphed = self.shading_copy();
        let mut used = Vec::new();
        for (index, key) in keys.iter().enumerate().skip(1) {
            if key.muted {
                continue;
            }
            let Some(reference) = self.shape_keys.reference(index) else {
                continue;
            };
            let group = key
                .vertex_group
                .as_deref()
                .and_then(|name| self.vertex_groups.index(name));
            let offsets: Vec<Vec3> = (0..self.point_count())
                .map(|point| {
                    let offset = key.positions[point] - reference.positions[point];
                    match group {
                        Some(group) => offset * self.vertex_groups.weight(point, group),
                        None => offset,
                    }
                })
                .collect();
            for ((position, base), offset) in morphed
                .positions
                .iter_mut()
                .zip(&self.positions)
                .zip(&offsets)
            {
                *position = *base + *offset;
            }
            let shaded = morphed.shade(shading);
            let mut target = MorphTarget::new(&key.name);
            for (vertex, &corner) in sources.iter().enumerate() {
                let point = self.corner_verts[corner as usize] as usize;
                let offset = offsets[point];
                let turned = shaded[corner as usize] - normals[corner as usize];
                if offset.length_squared() > MOVED * MOVED
                    || turned.length_squared() > MOVED * MOVED
                {
                    target.push(vertex as u32, offset, Some(turned));
                }
            }
            mesh.morph_targets.push(target);
            used.push(index as u32);
        }
        used
    }

    fn shading_copy(&self) -> Polymesh {
        let mut attributes = Attributes::new();
        for name in [SHARP_FACE, SHARP_EDGE, CUSTOM_NORMAL] {
            if let Some(attribute) = self.attributes.get(name) {
                attributes.insert(attribute.clone());
            }
        }
        Polymesh {
            positions: self.positions.clone(),
            edges: self.edges.clone(),
            face_offsets: self.face_offsets.clone(),
            corner_verts: self.corner_verts.clone(),
            corner_edges: self.corner_edges.clone(),
            attributes,
            ..Polymesh::default()
        }
    }
}
