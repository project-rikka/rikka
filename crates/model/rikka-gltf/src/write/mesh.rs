use std::collections::BTreeMap;

use rikka_model_core::glam::Vec3;

use crate::document::{
    Document, Index, Mesh as SourceDocument, MeshExtras, Primitive, Shape, TRIANGLES,
};
use crate::write::buffer::{Buffered, flatten};
use rikka_model_core::Model;
use rikka_model_core::{Mesh, MorphTarget};

pub(crate) fn write(
    model: &Model,
    document: &mut Document,
    buffered: &mut Buffered,
) -> Vec<Option<Index>> {
    let mut written = Vec::with_capacity(model.meshes.len());
    for mesh in &model.meshes {
        let described = described(mesh, buffered);
        written.push(match described.primitives.is_empty() {
            true => None,
            false => {
                document.meshes.push(described);
                Some((document.meshes.len() - 1) as Index)
            }
        });
    }
    written
}

fn described(mesh: &Mesh, buffered: &mut Buffered) -> SourceDocument {
    if !mesh.submeshes.iter().any(|submesh| submesh.index_count > 0) {
        return SourceDocument::default();
    }
    let attributes = attributes(mesh, buffered);
    let targets = targets(mesh, buffered);
    let names: Vec<String> = mesh
        .morph_targets
        .iter()
        .map(|target| target.name.clone())
        .collect();

    SourceDocument {
        primitives: mesh
            .submeshes
            .iter()
            .filter(|submesh| submesh.index_count > 0)
            .map(|submesh| {
                let first = submesh.first_index as usize;
                let indices = &mesh.indices[first..first + submesh.index_count as usize];
                Primitive {
                    attributes: attributes.clone(),
                    indices: Some(buffered.indices(indices)),
                    material: submesh.material.map(|material| material.0),
                    mode: TRIANGLES,
                    targets: targets.clone(),
                }
            })
            .collect(),
        weights: Vec::new(),
        name: Some(mesh.name.clone()).filter(|name| !name.is_empty()),
        extras: names
            .iter()
            .any(|name| !name.is_empty())
            .then_some(MeshExtras {
                target_names: names,
            }),
    }
}

fn attributes(mesh: &Mesh, buffered: &mut Buffered) -> BTreeMap<String, Index> {
    let vertices = &mesh.vertices;
    let mut attributes = BTreeMap::new();
    attributes.insert(
        "POSITION".to_owned(),
        buffered.positions(&vertices.positions),
    );
    let mut hold = |name: &str, values: Vec<f32>, shape: Shape| {
        if !values.is_empty() {
            attributes.insert(name.to_owned(), buffered.attribute(&values, shape));
        }
    };
    hold("NORMAL", flatten(&vertices.normals), Shape::Vec3);
    hold("TANGENT", flatten(&vertices.tangents), Shape::Vec4);
    hold("TEXCOORD_0", flatten(&vertices.uvs), Shape::Vec2);
    hold("WEIGHTS_0", flatten(&vertices.weights), Shape::Vec4);

    if !vertices.colors.is_empty() {
        attributes.insert("COLOR_0".to_owned(), buffered.colors(&vertices.colors));
    }
    if !vertices.joints.is_empty() {
        attributes.insert("JOINTS_0".to_owned(), buffered.joints(&vertices.joints));
    }
    attributes
}

fn targets(mesh: &Mesh, buffered: &mut Buffered) -> Vec<BTreeMap<String, Index>> {
    let count = mesh.vertices.len();
    let shaped = mesh
        .morph_targets
        .iter()
        .any(|target| !target.normals.is_empty());

    mesh.morph_targets
        .iter()
        .map(|target| {
            let mut attributes = BTreeMap::new();
            attributes.insert(
                "POSITION".to_owned(),
                sparse(buffered, target, &target.positions, count),
            );
            if shaped {
                attributes.insert(
                    "NORMAL".to_owned(),
                    sparse(buffered, target, &target.normals, count),
                );
            }
            attributes
        })
        .collect()
}

fn sparse(buffered: &mut Buffered, target: &MorphTarget, values: &[Vec3], count: usize) -> Index {
    match values.is_empty() {
        true => buffered.sparse(&[], &[], Shape::Vec3, count),
        false => buffered.sparse(&target.vertices, &flatten(values), Shape::Vec3, count),
    }
}
