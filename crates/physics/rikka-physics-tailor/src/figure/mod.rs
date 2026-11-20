mod skeleton;

use glam::{Mat3, Mat4, Vec3};
use rikka_humanoid::{Anatomy, Swing};
use rikka_model_core::{Bounds, INFLUENCES, Model, NodeId, Skin, Vertices};

pub(crate) use skeleton::Skeleton;

use crate::error::{Error, Result};
use crate::geometry::{area, is_proper};
use crate::key::VertexKey;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Part {
    pub(crate) node: NodeId,
    pub(crate) first: u32,
    pub(crate) len: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RestVertex {
    pub(crate) position: Vec3,
    pub(crate) normal: Vec3,
    pub(crate) tangent: Vec3,
    pub(crate) joints: [NodeId; INFLUENCES],
    pub(crate) weights: [f32; INFLUENCES],
    pub(crate) part: u32,
    pub(crate) weighted: bool,
}

impl RestVertex {
    pub(crate) fn strongest(&self) -> NodeId {
        self.joints[0]
    }

    pub(crate) fn share(&self, predicate: impl Fn(NodeId) -> bool) -> f32 {
        self.joints
            .iter()
            .zip(&self.weights)
            .filter(|(joint, _)| predicate(**joint))
            .map(|(_, weight)| weight)
            .sum()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RestTriangle {
    pub(crate) corners: [u32; 3],
    pub(crate) surface: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Surface {
    pub(crate) names: Vec<String>,
    pub(crate) mesh: Option<String>,
}

impl Surface {
    pub(crate) fn title(&self) -> &str {
        self.names
            .iter()
            .chain(&self.mesh)
            .find(|name| !name.trim().is_empty())
            .map_or("", |name| name.trim())
    }
}

#[derive(Debug)]
pub(crate) struct Figure<'m> {
    pub(crate) model: &'m Model,
    pub(crate) rest_worlds: Vec<Mat4>,
    pub(crate) parts: Vec<Part>,
    pub(crate) vertices: Vec<RestVertex>,
    pub(crate) triangles: Vec<RestTriangle>,
    pub(crate) surfaces: Vec<Surface>,
    pub(crate) skeleton: Skeleton,
    pub(crate) anatomy: &'m Anatomy,
    pub(crate) extent: f32,
}

impl<'m> Figure<'m> {
    pub(crate) fn new(model: &'m Model, anatomy: &'m Anatomy) -> Result<Figure<'m>> {
        model.validate().map_err(Error::Malformed)?;
        let rest_worlds = model.world_transforms();
        let aliases = model.material_aliases();
        let mut figure = Figure {
            model,
            parts: Vec::new(),
            vertices: Vec::new(),
            triangles: Vec::new(),
            surfaces: Vec::new(),
            skeleton: Skeleton::new(model, &rest_worlds, anatomy),
            anatomy,
            rest_worlds,
            extent: 0.0,
        };
        let meshes = model
            .nodes
            .iter()
            .filter(|node| node.mesh.is_some())
            .count();
        for (index, node) in model.nodes.iter().enumerate() {
            let Some(mesh_id) = node.mesh else {
                continue;
            };
            let node_id = NodeId(index as u32);
            let mesh = model.mesh(mesh_id);
            let first = figure.vertices.len() as u32;
            let part = figure.parts.len() as u32;
            figure.parts.push(Part {
                node: node_id,
                first,
                len: mesh.vertices.len() as u32,
            });
            let skin = node
                .skin
                .filter(|_| mesh.vertices.is_skinned())
                .map(|skin| model.skin(skin));
            figure.place(node_id, &mesh.vertices, skin, part);
            let named_mesh = (meshes > 1).then(|| mesh.name.clone());
            let mut ranges: Vec<(u32, u32, Vec<String>)> = mesh
                .submeshes
                .iter()
                .map(|submesh| {
                    let mut names = Vec::new();
                    if let Some(material) = submesh.material {
                        names.push(model.material(material).name.clone());
                        names.extend(
                            aliases
                                .iter()
                                .filter(|alias| alias.material == material)
                                .map(|alias| alias.name.clone()),
                        );
                    }
                    names.push(submesh.name.clone());
                    (submesh.first_index, submesh.index_count, names)
                })
                .collect();
            if ranges.is_empty() {
                ranges.push((0, mesh.indices.len() as u32, Vec::new()));
            }
            for (start, count, names) in ranges {
                let surface = figure.surfaces.len() as u32;
                figure.surfaces.push(Surface {
                    names,
                    mesh: named_mesh.clone(),
                });
                let end = (start as usize + count as usize).min(mesh.indices.len());
                let indices = mesh.indices.get(start as usize..end).unwrap_or(&[]);
                for corners in indices.as_chunks::<3>().0 {
                    if corners
                        .iter()
                        .any(|corner| *corner as usize >= mesh.vertices.len())
                    {
                        continue;
                    }
                    let corners = corners.map(|corner| corner + first);
                    if is_proper(corners.map(|corner| figure.vertices[corner as usize].position)) {
                        figure.triangles.push(RestTriangle { corners, surface });
                    }
                }
            }
            if mesh.vertices.normals.len() != mesh.vertices.len() {
                figure.estimate_normals(part);
            }
        }
        figure.extent = Bounds::from_points(figure.vertices.iter().map(|vertex| vertex.position))
            .longest()
            .max(1e-3);
        Ok(figure)
    }

    fn place(&mut self, node: NodeId, vertices: &Vertices, skin: Option<&Skin>, part: u32) {
        let world = self.rest_worlds[node.index()];
        let palette: Vec<Mat4> = skin.map_or_else(Vec::new, |skin| {
            skin.joints
                .iter()
                .zip(&skin.inverse_bind)
                .map(|(joint, bind)| self.rest_worlds[joint.index()] * *bind)
                .collect()
        });
        for (index, position) in vertices.positions.iter().enumerate() {
            let influences = skin.and_then(|skin| bound(vertices, index, skin));
            let (joints, weights, matrix) = match influences {
                Some(influences) => {
                    let matrix = influences
                        .iter()
                        .filter(|(_, weight)| *weight > 0.0)
                        .fold(Mat4::ZERO, |sum, (slot, weight)| {
                            sum + palette[*slot] * *weight
                        });
                    let joints = influences.map(|(slot, weight)| {
                        if weight > 0.0 {
                            skin.map_or(node, |skin| skin.joints[slot])
                        } else {
                            node
                        }
                    });
                    let strongest = joints[0];
                    let joints = std::array::from_fn(|slot| {
                        if influences[slot].1 > 0.0 {
                            joints[slot]
                        } else {
                            strongest
                        }
                    });
                    (joints, influences.map(|(_, weight)| weight), matrix)
                }
                None => {
                    let mut weights = [0.0; INFLUENCES];
                    weights[0] = 1.0;
                    ([node; INFLUENCES], weights, world)
                }
            };
            let turn = Mat3::from_mat4(matrix);
            let normal = vertices
                .normals
                .get(index)
                .map_or(Vec3::ZERO, |normal| (turn * *normal).normalize_or_zero());
            let tangent = vertices.tangents.get(index).map_or(Vec3::ZERO, |tangent| {
                (turn * tangent.truncate()).normalize_or_zero()
            });
            self.vertices.push(RestVertex {
                position: matrix.transform_point3(*position),
                normal,
                tangent,
                joints,
                weights,
                part,
                weighted: skin.is_none() || influences.is_some(),
            });
        }
    }

    fn estimate_normals(&mut self, part: u32) {
        let Part { first, len, .. } = self.parts[part as usize];
        let range = first as usize..(first + len) as usize;
        let mut sums = vec![Vec3::ZERO; len as usize];
        for triangle in &self.triangles {
            if !range.contains(&(triangle.corners[0] as usize)) {
                continue;
            }
            let corners = triangle
                .corners
                .map(|corner| self.vertices[corner as usize].position);
            let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            for corner in triangle.corners {
                sums[(corner - first) as usize] += normal;
            }
        }
        for (vertex, sum) in self.vertices[range].iter_mut().zip(sums) {
            vertex.normal = sum.normalize_or_zero();
        }
    }

    pub(crate) fn hung(&self, node: NodeId) -> Option<&Swing> {
        self.anatomy.swing(node).filter(|swing| !swing.is_flesh())
    }

    pub(crate) fn key(&self, vertex: u32) -> VertexKey {
        let part = &self.parts[self.vertices[vertex as usize].part as usize];
        VertexKey::new(part.node, vertex - part.first)
    }

    pub(crate) fn index(&self, key: VertexKey) -> Option<u32> {
        self.parts
            .iter()
            .find(|part| part.node == key.node)
            .filter(|part| key.vertex < part.len)
            .map(|part| part.first + key.vertex)
    }

    pub(crate) fn corners(&self, triangle: &RestTriangle) -> [Vec3; 3] {
        triangle
            .corners
            .map(|corner| self.vertices[corner as usize].position)
    }

    pub(crate) fn triangle_area(&self, triangle: &RestTriangle) -> f32 {
        area(self.corners(triangle))
    }

    pub(crate) fn same_distance(&self) -> f32 {
        self.extent * 0.001
    }
}

fn bound(vertices: &Vertices, vertex: usize, skin: &Skin) -> Option<[(usize, f32); INFLUENCES]> {
    let raw = vertices.influences(vertex)?;
    let joints = skin.joints.len();
    let counted =
        |slot: u16, weight: f32| weight > 0.0 && weight.is_finite() && usize::from(slot) < joints;
    let total: f32 = raw
        .iter()
        .filter(|(slot, weight)| counted(*slot, *weight))
        .map(|(_, weight)| weight)
        .sum();
    if total <= 1e-6 {
        return None;
    }
    let mut weights: [(usize, f32); INFLUENCES] = raw.map(|(slot, weight)| {
        if counted(slot, weight) {
            (usize::from(slot), weight / total)
        } else {
            (0, 0.0)
        }
    });
    for slot in 0..INFLUENCES {
        for other in slot + 1..INFLUENCES {
            if weights[other].1 > 0.0
                && weights[other].0 == weights[slot].0
                && weights[slot].1 > 0.0
            {
                weights[slot].1 += weights[other].1;
                weights[other] = (0, 0.0);
            }
        }
    }
    weights.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    Some(weights)
}
