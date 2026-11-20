use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};

use rikka_gltf::GltfImporter;
use rikka_mmd::PmxImporter;
use rikka_model_core::{Import, Registry};
use rikka_polymesh::{FromMesh, Polymesh};
use rikka_vrm::VrmImporter;

use crate::corpus::Fixture;

const SAMPLES: [(&str, &str); 3] = [
    ("miku", "crates/model/rikka-mmd/assets/mmd/miku.pmx"),
    ("avatar", "crates/model/rikka-vrm/assets/avatar.vrm"),
    ("fox", "crates/model/rikka-gltf/assets/fox.glb"),
];

#[must_use]
pub fn sample_meshes() -> &'static [Fixture] {
    static MESHES: OnceLock<Vec<Fixture>> = OnceLock::new();
    MESHES.get_or_init(load)
}

fn load() -> Vec<Fixture> {
    let registry = Arc::new(
        Registry::new()
            .with_importer(PmxImporter)
            .with_importer(VrmImporter)
            .with_importer(GltfImporter),
    );
    let mut meshes = Vec::new();
    for (name, path) in SAMPLES {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => panic!("failed to read {path}: {error}"),
        };
        let model = match Import::named_bytes(path, bytes, Arc::clone(&registry)).run() {
            Ok(model) => model,
            Err(error) => panic!("failed to import {path}: {error}"),
        };
        let mut parts: Vec<Fixture> = model
            .meshes
            .iter()
            .enumerate()
            .map(|(index, mesh)| {
                Fixture::new(
                    format!("{name}_{index}"),
                    Polymesh::from_mesh(mesh, &FromMesh::new()),
                )
            })
            .filter(|fixture| fixture.mesh.face_count() > 0)
            .collect();
        parts.sort_by_key(|fixture| std::cmp::Reverse(fixture.mesh.face_count()));
        meshes.extend(parts);
    }
    meshes
}

#[must_use]
pub fn sample_patches(faces: usize, per_sample: usize) -> Vec<Fixture> {
    let mut patches = Vec::new();
    for (name, _) in SAMPLES {
        let prefix = format!("{name}_");
        for sample in sample_meshes()
            .iter()
            .filter(|fixture| fixture.name.starts_with(&prefix))
            .take(per_sample)
        {
            let region = grow(&sample.mesh, sample.mesh.face_count() / 2, faces);
            patches.push(Fixture::new(
                format!("{}_patch", sample.name),
                sample.mesh.extract(&region),
            ));
        }
    }
    patches
}

fn grow(mesh: &Polymesh, seed: usize, limit: usize) -> Vec<u32> {
    let mut edge_faces: Vec<Vec<u32>> = vec![Vec::new(); mesh.edge_count()];
    for face in 0..mesh.face_count() {
        for corner in mesh.face(face) {
            edge_faces[mesh.corner_edges[corner] as usize].push(face as u32);
        }
    }
    let mut taken = vec![false; mesh.face_count()];
    let mut region = Vec::new();
    let mut queue = VecDeque::from([seed as u32]);
    taken[seed] = true;
    while let Some(face) = queue.pop_front() {
        region.push(face);
        if region.len() >= limit {
            break;
        }
        for corner in mesh.face(face as usize) {
            for &other in &edge_faces[mesh.corner_edges[corner] as usize] {
                if !taken[other as usize] {
                    taken[other as usize] = true;
                    queue.push_back(other);
                }
            }
        }
    }
    region.sort_unstable();
    region
}
