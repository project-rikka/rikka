use glam::{Vec2, Vec3, Vec4};
use rikka_model_core::{Mesh, Polygons};
use rustc_hash::FxHashMap;

use crate::domain::Domain;
use crate::polymesh::Polymesh;
use crate::shape_keys::ShapeKey;
use crate::vertex_groups::{Influence, VertexGroup};

const SAME_NORMAL: f32 = 1.0e-4;
const DEFAULT_UV_MAP: &str = "UVMap";
const DEFAULT_COLOR_ATTRIBUTE: &str = "Color";
const BASIS: &str = "Basis";

#[derive(Clone, Debug)]
#[must_use]
pub struct FromMesh {
    joint_names: Vec<String>,
    uv_map: String,
    color_attribute: String,
}

impl Default for FromMesh {
    fn default() -> FromMesh {
        FromMesh {
            joint_names: Vec::new(),
            uv_map: DEFAULT_UV_MAP.to_owned(),
            color_attribute: DEFAULT_COLOR_ATTRIBUTE.to_owned(),
        }
    }
}

impl FromMesh {
    pub fn new() -> FromMesh {
        FromMesh::default()
    }

    pub fn joint_names(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> FromMesh {
        self.joint_names = names.into_iter().map(Into::into).collect();
        self
    }

    pub fn uv_map(mut self, name: impl Into<String>) -> FromMesh {
        self.uv_map = name.into();
        self
    }

    pub fn color_attribute(mut self, name: impl Into<String>) -> FromMesh {
        self.color_attribute = name.into();
        self
    }
}

#[derive(PartialEq, Eq, Hash)]
struct PointKey {
    position: [u32; 3],
    joints: [u16; 4],
    weights: [u32; 4],
    deltas: Vec<[u32; 3]>,
}

fn bits(values: &[f32]) -> impl Iterator<Item = u32> + '_ {
    values.iter().map(|value| (value + 0.0).to_bits())
}

impl Polymesh {
    #[must_use]
    pub fn from_mesh(mesh: &Mesh, options: &FromMesh) -> Polymesh {
        let vertices = &mesh.vertices;
        let deltas = morph_deltas(mesh);
        let (vertex_points, offsets, corner_vertices, face_submeshes) = match usable_polygons(mesh)
        {
            Some(polygons) => (
                polygons.vertex_points.clone(),
                polygons.offsets.clone(),
                polygons.corner_vertices.clone(),
                polygons.submeshes.clone(),
            ),
            None => {
                let points = weld_vertices(mesh, &deltas);
                let all = mesh.indices.len() / 3;
                let mut submesh_of = vec![0u32; all];
                for (index, submesh) in mesh.submeshes.iter().enumerate() {
                    let first = (submesh.first_index as usize / 3).min(all);
                    let end = (first + submesh.index_count as usize / 3).min(all);
                    submesh_of[first..end].fill(index as u32);
                }
                let kept: Vec<usize> = (0..all)
                    .filter(|&triangle| {
                        mesh.indices[triangle * 3..triangle * 3 + 3]
                            .iter()
                            .all(|&vertex| (vertex as usize) < vertices.len())
                    })
                    .collect();
                (
                    points,
                    (0..=kept.len() as u32)
                        .map(|triangle| triangle * 3)
                        .collect(),
                    kept.iter()
                        .flat_map(|&triangle| {
                            mesh.indices[triangle * 3..triangle * 3 + 3].iter().copied()
                        })
                        .collect(),
                    kept.iter().map(|&triangle| submesh_of[triangle]).collect(),
                )
            }
        };

        let point_count = vertex_points
            .iter()
            .max()
            .map_or(0, |&point| point as usize + 1);
        let mut first_vertex = vec![u32::MAX; point_count];
        for (vertex, &point) in vertex_points.iter().enumerate() {
            let first = &mut first_vertex[point as usize];
            if *first == u32::MAX {
                *first = vertex as u32;
            }
        }
        let positions: Vec<Vec3> = first_vertex
            .iter()
            .map(|&vertex| {
                vertices
                    .positions
                    .get(vertex as usize)
                    .copied()
                    .unwrap_or(Vec3::ZERO)
            })
            .collect();
        let mut polymesh = Polymesh {
            positions,
            face_offsets: offsets,
            corner_verts: corner_vertices
                .iter()
                .map(|&vertex| vertex_points[vertex as usize])
                .collect(),
            ..Polymesh::default()
        };
        polymesh.ensure_edges();
        if !face_submeshes.is_empty() {
            polymesh
                .material_indices_mut()
                .iter_mut()
                .zip(&face_submeshes)
                .for_each(|(index, &submesh)| *index = submesh as i32);
        }

        if vertices.uvs.len() == vertices.len() && !vertices.uvs.is_empty() {
            let uvs = corner_vertices
                .iter()
                .map(|&vertex| {
                    let uv = vertices.uvs[vertex as usize];
                    Vec2::new(uv.x, 1.0 - uv.y)
                })
                .collect();
            polymesh.add_uv_map(options.uv_map.clone(), uvs);
        }
        if vertices.colors.len() == vertices.len() && !vertices.colors.is_empty() {
            let colors = corner_vertices
                .iter()
                .map(|&vertex| {
                    Vec4::from_array(
                        vertices.colors[vertex as usize].map(|channel| f32::from(channel) / 255.0),
                    )
                })
                .collect();
            polymesh.add_color_attribute(options.color_attribute.clone(), Domain::Corner, colors);
        }
        polymesh.groups_from_skin(mesh, &first_vertex, &options.joint_names);
        polymesh.keys_from_morphs(mesh, &first_vertex, &deltas);
        if vertices.normals.len() == vertices.len() && !vertices.normals.is_empty() {
            let normals: Vec<Vec3> = corner_vertices
                .iter()
                .map(|&vertex| vertices.normals[vertex as usize])
                .collect();
            polymesh.adopt_normals(&normals);
        }
        polymesh
    }

    fn groups_from_skin(&mut self, mesh: &Mesh, first_vertex: &[u32], joint_names: &[String]) {
        let vertices = &mesh.vertices;
        if !vertices.is_skinned() || joint_names.is_empty() {
            return;
        }
        self.vertex_groups.groups = joint_names.iter().map(VertexGroup::new).collect();
        self.vertex_groups.active = Some(0);
        self.vertex_groups.weights = first_vertex
            .iter()
            .map(|&vertex| {
                let Some(influences) = vertices.influences(vertex as usize) else {
                    return Vec::new();
                };
                let mut list: Vec<Influence> = Vec::new();
                for (joint, weight) in influences {
                    if weight <= 0.0 || joint as usize >= joint_names.len() {
                        continue;
                    }
                    match list
                        .iter_mut()
                        .find(|influence| influence.group == u32::from(joint))
                    {
                        Some(influence) => influence.weight += weight,
                        None => list.push(Influence {
                            group: u32::from(joint),
                            weight,
                        }),
                    }
                }
                list
            })
            .collect();
    }

    fn keys_from_morphs(&mut self, mesh: &Mesh, first_vertex: &[u32], deltas: &[Vec<Vec3>]) {
        if mesh.morph_targets.is_empty() {
            return;
        }
        let basis = ShapeKey::new(BASIS, self.positions.clone());
        self.shape_keys.add(basis);
        for (target, deltas) in mesh.morph_targets.iter().zip(deltas) {
            let positions = first_vertex
                .iter()
                .zip(&self.positions)
                .map(|(&vertex, &position)| {
                    position + deltas.get(vertex as usize).copied().unwrap_or(Vec3::ZERO)
                })
                .collect();
            self.shape_keys
                .add(ShapeKey::new(target.name.clone(), positions));
        }
    }

    fn adopt_normals(&mut self, normals: &[Vec3]) {
        let normals: Vec<Vec3> = normals
            .iter()
            .map(|normal| normal.normalize_or_zero())
            .collect();
        let differ = |a: Vec3, b: Vec3| (a - b).abs().max_element() > SAME_NORMAL;
        let face_normals = self.face_normals();
        let flat: Vec<bool> = (0..self.face_count())
            .map(|face| {
                self.face(face)
                    .all(|corner| !differ(normals[corner], face_normals[face]))
            })
            .collect();
        if flat.contains(&true) {
            self.sharp_faces_mut().copy_from_slice(&flat);
        }
        let owners = self.corner_faces();
        let edge_corners = self.edge_corners();
        let mut sharp = vec![false; self.edge_count()];
        for (edge, flag) in sharp.iter_mut().enumerate() {
            let corners = edge_corners.get(edge);
            let [first, second] = corners else {
                continue;
            };
            let (first, second) = (*first as usize, *second as usize);
            let next_first = self.next_corner(owners[first] as usize, first);
            let next_second = self.next_corner(owners[second] as usize, second);
            let (a_first, a_second) = match self.corner_verts[first] == self.corner_verts[second] {
                true => ((first, second), (next_first, next_second)),
                false => ((first, next_second), (next_first, second)),
            };
            *flag = differ(normals[a_first.0], normals[a_first.1])
                || differ(normals[a_second.0], normals[a_second.1]);
        }
        if sharp.contains(&true) {
            self.sharp_edges_mut().copy_from_slice(&sharp);
        }
        let derived = self.corner_normals();
        if derived
            .iter()
            .zip(&normals)
            .any(|(derived, given)| differ(*derived, *given))
        {
            self.custom_normals_mut().copy_from_slice(&normals);
        }
    }
}

fn usable_polygons(mesh: &Mesh) -> Option<&Polygons> {
    let polygons = mesh.polygons.as_ref()?;
    let vertices = mesh.vertices.len();
    let sound = polygons.vertex_points.len() == vertices
        && polygons.offsets.first() == Some(&0)
        && polygons.offsets.last().map(|&last| last as usize)
            == Some(polygons.corner_vertices.len())
        && polygons
            .offsets
            .windows(2)
            .all(|pair| u64::from(pair[1]) >= u64::from(pair[0]) + 3)
        && polygons.submeshes.len() == polygons.len()
        && polygons
            .corner_vertices
            .iter()
            .all(|&vertex| (vertex as usize) < vertices);
    sound.then_some(polygons)
}

fn morph_deltas(mesh: &Mesh) -> Vec<Vec<Vec3>> {
    let count = mesh.vertices.len();
    mesh.morph_targets
        .iter()
        .map(|target| {
            let mut deltas = vec![Vec3::ZERO; count];
            for (&vertex, &delta) in target.vertices.iter().zip(&target.positions) {
                if let Some(slot) = deltas.get_mut(vertex as usize) {
                    *slot = delta;
                }
            }
            deltas
        })
        .collect()
}

fn weld_vertices(mesh: &Mesh, deltas: &[Vec<Vec3>]) -> Vec<u32> {
    let vertices = &mesh.vertices;
    let mut points: FxHashMap<PointKey, u32> = FxHashMap::default();
    (0..vertices.len())
        .map(|vertex| {
            let influences = vertices.influences(vertex);
            let key = PointKey {
                position: std::array::from_fn(|axis| {
                    (vertices.positions[vertex][axis] + 0.0).to_bits()
                }),
                joints: influences.map_or([0; 4], |influences| influences.map(|(joint, _)| joint)),
                weights: influences.map_or([0; 4], |influences| {
                    influences.map(|(_, weight)| (weight + 0.0).to_bits())
                }),
                deltas: deltas
                    .iter()
                    .map(|deltas| {
                        let mut delta = [0u32; 3];
                        for (slot, value) in delta.iter_mut().zip(bits(&deltas[vertex].to_array()))
                        {
                            *slot = value;
                        }
                        delta
                    })
                    .collect(),
            };
            let next = points.len() as u32;
            *points.entry(key).or_insert(next)
        })
        .collect()
}
