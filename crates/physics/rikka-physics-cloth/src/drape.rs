use rikka_gpu::Compute;
use rikka_gpu::{ComputeBuffer, Ticket};
use rikka_model_core::{Model, NodeId};
use rikka_physics_gpu::Ring;
use rikka_renderer_3d::DRAPE_VERTEX_BYTES;

use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub struct Drape {
    pub node: NodeId,
    pub vertices: u32,
    pub buffer: ComputeBuffer,
    pub ready: Ticket,
}

#[derive(Debug)]
struct Output {
    node: NodeId,
    vertices: u32,
    ring: Ring,
    current: Option<ComputeBuffer>,
}

#[derive(Debug)]
pub(crate) struct Outputs {
    outputs: Vec<Output>,
    ready: Option<Ticket>,
}

impl Outputs {
    pub(crate) fn new(model: &Model, nodes: &[NodeId], bound: &[(NodeId, u32)]) -> Result<Outputs> {
        let mut outputs = Vec::with_capacity(nodes.len());
        for &node in nodes {
            let vertices = model
                .nodes
                .get(node.index())
                .and_then(|described| described.mesh)
                .map(|mesh| model.mesh(mesh).vertices.len() as u32)
                .ok_or(Error::MissingMesh(node))?;
            if let Some((_, vertex)) = bound
                .iter()
                .find(|(bound_node, vertex)| *bound_node == node && *vertex >= vertices)
            {
                return Err(Error::MissingVertex {
                    node,
                    vertex: *vertex,
                });
            }
            outputs.push(Output {
                node,
                vertices,
                ring: Ring::new(u64::from(vertices) * DRAPE_VERTEX_BYTES),
                current: None,
            });
        }
        Ok(Outputs {
            outputs,
            ready: None,
        })
    }

    pub(crate) fn acquire(&mut self, compute: &Compute) -> Result<Vec<ComputeBuffer>> {
        self.outputs
            .iter_mut()
            .map(|output| {
                let buffer = output.ring.acquire(compute)?;
                output.current = Some(buffer.clone());
                Ok(buffer)
            })
            .collect()
    }

    pub(crate) fn vertices(&self, slot: usize) -> u32 {
        self.outputs.get(slot).map_or(0, |output| output.vertices)
    }

    pub(crate) fn present(&mut self, ready: Ticket) {
        self.ready = Some(ready);
    }

    pub(crate) fn recorded(&self) -> impl Iterator<Item = (NodeId, &ComputeBuffer)> {
        self.outputs
            .iter()
            .filter_map(|output| output.current.as_ref().map(|buffer| (output.node, buffer)))
    }

    pub(crate) fn drapes(&self) -> Vec<Drape> {
        let Some(ready) = &self.ready else {
            return Vec::new();
        };
        self.outputs
            .iter()
            .filter_map(|output| {
                output.current.as_ref().map(|buffer| Drape {
                    node: output.node,
                    vertices: output.vertices,
                    buffer: buffer.clone(),
                    ready: ready.clone(),
                })
            })
            .collect()
    }
}

pub(crate) fn handedness(model: &Model, bound: &[(NodeId, u32)]) -> Vec<f32> {
    bound
        .iter()
        .map(|&(node, vertex)| {
            model
                .nodes
                .get(node.index())
                .and_then(|described| described.mesh)
                .and_then(|mesh| model.mesh(mesh).vertices.tangents.get(vertex as usize))
                .map_or(1.0, |tangent| if tangent.w < 0.0 { -1.0 } else { 1.0 })
        })
        .collect()
}
