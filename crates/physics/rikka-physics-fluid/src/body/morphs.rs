use rikka_model_core::Mesh;

use crate::error::{Error, Result};

#[derive(Clone, Debug, Default)]
pub(crate) struct Morphs {
    pub(crate) starts: Vec<u32>,
    pub(crate) deltas: Vec<[f32; 4]>,
    reach: Vec<f32>,
    moved: Vec<(u32, u32, [f32; 3])>,
}

impl Morphs {
    pub(crate) fn add(&mut self, mesh: &Mesh, first_vertex: u32) -> Result<()> {
        let vertices = mesh.vertices.len();
        for target in &mesh.morph_targets {
            if target.vertices.len() != target.positions.len() {
                return Err(Error::NotABody(format!(
                    "morph target \"{}\" of mesh \"{}\" moves {} vertices by {} offsets",
                    target.name,
                    mesh.name,
                    target.vertices.len(),
                    target.positions.len()
                )));
            }
            let index = self.reach.len() as u32;
            let mut reach = 0.0f32;
            for (&vertex, offset) in target.vertices.iter().zip(&target.positions) {
                if vertex as usize >= vertices {
                    return Err(Error::NotABody(format!(
                        "morph target \"{}\" moves vertex {vertex} of mesh \"{}\" with {vertices}",
                        target.name, mesh.name
                    )));
                }
                if !offset.is_finite() {
                    return Err(Error::NotABody(format!(
                        "morph target \"{}\" of mesh \"{}\" holds a non-finite offset",
                        target.name, mesh.name
                    )));
                }
                reach = reach.max(offset.length());
                self.moved
                    .push((first_vertex + vertex, index, offset.to_array()));
            }
            self.reach.push(reach);
        }
        Ok(())
    }

    pub(crate) fn finish(&mut self, vertices: u32) {
        let mut moved = std::mem::take(&mut self.moved);
        moved.sort_by_key(|&(vertex, target, _)| (vertex, target));
        self.starts = vec![0; vertices as usize + 1];
        for &(vertex, _, _) in &moved {
            self.starts[vertex as usize + 1] += 1;
        }
        for at in 1..self.starts.len() {
            self.starts[at] += self.starts[at - 1];
        }
        self.deltas = moved
            .into_iter()
            .map(|(_, target, [x, y, z])| [x, y, z, target as f32])
            .collect();
    }

    pub(crate) fn targets(&self) -> u32 {
        self.reach.len() as u32
    }

    pub(crate) fn travel(&self, from: &[f32], to: &[f32]) -> f32 {
        self.reach
            .iter()
            .zip(from.iter().zip(to))
            .map(|(reach, (before, after))| reach * (after - before).abs())
            .sum()
    }
}
