use glam::{Vec3, Vec4};
use rikka_model_core::{Model, NodeId, Vertices};

use crate::body::morphs::Morphs;
use crate::body::reach::JointReach;
use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Part {
    pub(crate) node: NodeId,
    pub(crate) first_vertex: u32,
    pub(crate) vertices: u32,
    pub(crate) first_triangle: u32,
    pub(crate) triangles: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct BodyMesh {
    pub(crate) rest: Vec<[f32; 4]>,
    pub(crate) joints: Vec<[u32; 4]>,
    pub(crate) weights: Vec<[u32; 2]>,
    pub(crate) triangles: Vec<[u32; 3]>,
    pub(crate) parts: Vec<Part>,
    pub(crate) palette: u32,
    pub(crate) areas: Vec<f32>,
    pub(crate) reach: Vec<JointReach>,
    pub(crate) morphs: Morphs,
}

impl BodyMesh {
    pub(crate) fn flatten(model: &Model) -> Result<BodyMesh> {
        let mut body = BodyMesh {
            rest: Vec::new(),
            joints: Vec::new(),
            weights: Vec::new(),
            triangles: Vec::new(),
            parts: Vec::new(),
            palette: 0,
            areas: Vec::new(),
            reach: Vec::new(),
            morphs: Morphs::default(),
        };
        for (index, node) in model.nodes.iter().enumerate() {
            let Some(mesh) = node.mesh else {
                continue;
            };
            let mesh = model.mesh(mesh);
            let vertices = &mesh.vertices;
            let skinned = node.skin.is_some() && vertices.is_skinned();
            let first_vertex = body.rest.len() as u32;
            let first_triangle = body.triangles.len() as u32;
            let base_joint = body.palette;
            let joint_count = match (node.skin, skinned) {
                (Some(skin), true) => model.skin(skin).joint_count() as u32,
                _ => 1,
            };
            for vertex in 0..vertices.len() {
                let position = vertices.positions[vertex];
                body.rest.push([position.x, position.y, position.z, 0.0]);
                let (joints, weights) = if skinned {
                    influences(vertices, vertex, joint_count)?
                } else {
                    ([0; 4], [65535, 0])
                };
                body.joints.push(joints.map(|joint| joint + base_joint));
                body.weights.push(weights);
            }
            for triangle in mesh.indices.as_chunks::<3>().0 {
                if triangle.iter().any(|&at| at as usize >= vertices.len()) {
                    return Err(Error::NotABody(format!(
                        "mesh \"{}\" indexes past its {} vertices",
                        mesh.name,
                        vertices.len()
                    )));
                }
                let corners = triangle.map(|at| at + first_vertex);
                body.areas.push(area(&body.rest, corners));
                body.triangles.push(corners);
            }
            body.morphs.add(mesh, first_vertex)?;
            body.parts.push(Part {
                node: NodeId(index as u32),
                first_vertex,
                vertices: vertices.len() as u32,
                first_triangle,
                triangles: body.triangles.len() as u32 - first_triangle,
            });
            body.palette += joint_count;
        }
        if body.triangles.is_empty() {
            return Err(Error::NotABody("it holds no triangle".into()));
        }
        body.morphs.finish(body.vertex_count());
        body.reach = JointReach::compute(&body);
        Ok(body)
    }

    pub(crate) fn vertex_count(&self) -> u32 {
        self.rest.len() as u32
    }

    pub(crate) fn triangle_count(&self) -> u32 {
        self.triangles.len() as u32
    }

    pub(crate) fn part(&self, node: NodeId) -> Option<&Part> {
        self.parts.iter().find(|part| part.node == node)
    }

    pub(crate) fn weight_of(&self, vertex: usize, slot: usize) -> f32 {
        let packed = self.weights[vertex][slot / 2];
        let bits = match slot % 2 {
            0 => packed & 0xffff,
            _ => packed >> 16,
        };
        bits as f32 / 65535.0
    }
}

fn influences(
    vertices: &Vertices,
    vertex: usize,
    joint_count: u32,
) -> Result<([u32; 4], [u32; 2])> {
    let joints = vertices.joints.get(vertex).copied().unwrap_or([0; 4]);
    let weights = vertices.weights.get(vertex).copied().unwrap_or(Vec4::X);
    let quantized = quantized(weights);
    for (joint, weight) in joints.iter().zip(quantized) {
        if weight > 0 && u32::from(*joint) >= joint_count {
            return Err(Error::NotABody(format!(
                "vertex references joint {joint} of a skin with {joint_count} joints"
            )));
        }
    }
    Ok((
        joints.map(u32::from),
        [
            u32::from(quantized[0]) | (u32::from(quantized[1]) << 16),
            u32::from(quantized[2]) | (u32::from(quantized[3]) << 16),
        ],
    ))
}

fn quantized(weights: Vec4) -> [u16; 4] {
    let sum = weights.x + weights.y + weights.z + weights.w;
    let normalized = if sum > 1e-6 { weights / sum } else { Vec4::X };
    let mut quantized = normalized.to_array().map(unorm16);
    let total: u32 = quantized.iter().map(|&w| u32::from(w)).sum();
    let largest = quantized
        .iter()
        .enumerate()
        .max_by_key(|&(_, &w)| w)
        .map_or(0, |(index, _)| index);
    let corrected = i64::from(quantized[largest]) + 65535 - i64::from(total);
    quantized[largest] = corrected.clamp(0, 65535) as u16;
    quantized
}

fn unorm16(value: f32) -> u16 {
    (value.clamp(0.0, 1.0) * 65535.0).round() as u16
}

fn area(rest: &[[f32; 4]], corners: [u32; 3]) -> f32 {
    let [a, b, c] = corners.map(|at| Vec3::from_slice(&rest[at as usize][..3]));
    0.5 * (b - a).cross(c - a).length()
}
