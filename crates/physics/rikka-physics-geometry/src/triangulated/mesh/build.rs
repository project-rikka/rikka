use rikka_physics_math::{Aabb, Vec3};

use super::data::{MeshParts, TriangleMesh};
use super::def::MeshDef;
use super::edges::{identify_edges, is_closed};
use crate::triangulated::bvh::Bvh4;
use crate::{Error, Result, Tolerance};

const LEAF_SIZE: u32 = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct MeshBuild {
    pub mesh: TriangleMesh,
    pub degenerate: Vec<u32>,
}

impl TriangleMesh {
    pub fn build(def: &MeshDef<'_>) -> Result<MeshBuild> {
        validate(def)?;
        let slop = Tolerance::default().linear_slop();
        let min_area = 0.01 * slop * slop;
        let source: Vec<[u32; 3]> = def
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|chunk| {
                if def.clockwise {
                    [chunk[0], chunk[2], chunk[1]]
                } else {
                    [chunk[0], chunk[1], chunk[2]]
                }
            })
            .collect();
        let mut bounds = vec![Aabb::EMPTY; source.len()];
        let mut usable = Vec::with_capacity(source.len());
        let mut degenerate = Vec::new();
        let mut surface_area = 0.0;
        for (index, triangle) in source.iter().enumerate() {
            let [a, b, c] = triangle.map(|vertex| def.vertices[vertex as usize]);
            let area = 0.5 * (b - a).cross(c - a).length();
            if area < min_area {
                degenerate.push(index as u32);
                continue;
            }
            surface_area += area;
            bounds[index] = Aabb::new(a.min(b).min(c), a.max(b).max(c));
            usable.push(index as u32);
        }
        if usable.is_empty() {
            return Err(Error::EmptyMesh);
        }
        let mut builder = Bvh4::builder().leaf_size(LEAF_SIZE);
        if let Some(executor) = def.executor {
            builder = builder.executor(executor);
        }
        let tree = builder.build(&bounds, usable);
        let quantized = tree.quantize();
        let order = tree.items();
        let triangles: Vec<[u32; 3]> = order.iter().map(|&index| source[index as usize]).collect();
        let materials: Vec<u8> = if def.materials.is_empty() {
            vec![0; order.len()]
        } else {
            order
                .iter()
                .map(|&index| def.materials[index as usize])
                .collect()
        };
        let material_count = materials
            .iter()
            .copied()
            .max()
            .map_or(1, |max| u32::from(max) + 1);
        let flags = if def.identify_edges {
            identify_edges(def.vertices, &triangles)
        } else {
            vec![0; triangles.len()]
        };
        let mesh = TriangleMesh::assemble(MeshParts {
            bounds: quantized.bounds,
            surface_area,
            material_count,
            closed: is_closed(&triangles),
            degenerate_count: degenerate.len() as u32,
            nodes: &quantized.nodes,
            vertices: def.vertices,
            triangles: &triangles,
            materials: &materials,
            flags: &flags,
        });
        Ok(MeshBuild { mesh, degenerate })
    }
}

fn validate(def: &MeshDef<'_>) -> Result<()> {
    if !def.indices.len().is_multiple_of(3) {
        return Err(Error::IndexCount {
            count: def.indices.len(),
        });
    }
    if let Some(&index) = def
        .indices
        .iter()
        .find(|&&index| index as usize >= def.vertices.len())
    {
        return Err(Error::IndexOutOfRange {
            index,
            vertices: def.vertices.len(),
        });
    }
    if !def.vertices.iter().all(|vertex: &Vec3| vertex.is_finite()) {
        return Err(Error::NonFinite {
            what: "mesh vertex",
        });
    }
    let triangles = def.indices.len() / 3;
    if !def.materials.is_empty() && def.materials.len() != triangles {
        return Err(Error::MaterialCount {
            expected: triangles,
            actual: def.materials.len(),
        });
    }
    Ok(())
}
