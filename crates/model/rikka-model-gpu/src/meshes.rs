use rikka_model_core::glam::Vec3;
use rikka_renderer::Renderer;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::{
    Indices, MeshData, MeshId as GpuMesh, MorphTarget as GpuMorph, Submesh as GpuSubmesh,
};

use crate::error::{Error, Result};
use rikka_model_core::Mesh;

pub(crate) fn upload(
    renderer: &mut Renderer,
    mesh: &Mesh,
    outline_normals: &[Vec3],
) -> Result<GpuMesh> {
    let submeshes: Vec<GpuSubmesh> = mesh
        .submeshes
        .iter()
        .map(|submesh| GpuSubmesh {
            first_index: submesh.first_index,
            index_count: submesh.index_count,
            center: submesh.bounds.center(),
            radius: submesh.bounds.radius(),
        })
        .collect();
    let morphs: Vec<GpuMorph<'_>> = mesh
        .morph_targets
        .iter()
        .map(|target| GpuMorph {
            vertices: &target.vertices,
            positions: &target.positions,
            normals: &target.normals,
        })
        .collect();
    let vertices = &mesh.vertices;
    let data = MeshData {
        positions: &vertices.positions,
        normals: &vertices.normals,
        tangents: &vertices.tangents,
        uvs: &vertices.uvs,
        colors: &vertices.colors,
        joints: &vertices.joints,
        weights: &vertices.weights,
        outline_normals,
        indices: Some(Indices::U32(&mesh.indices)),
        submeshes: &submeshes,
        morph_targets: &morphs,
    };
    renderer.three().create_mesh(&data).map_err(Error::mesh)
}
