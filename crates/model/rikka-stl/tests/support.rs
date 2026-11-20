use std::sync::Arc;

use rikka_model_core::glam::Vec3;
use rikka_model_core::{Export, Import, Model, Registry};
use rikka_stl::{StlExporter, StlImporter};

pub(crate) const TEAPOT: &[u8] = include_bytes!("../assets/teapot.stl");
pub(crate) const CUBES: &[u8] = include_bytes!("../assets/cubes.stl");

pub(crate) fn registry() -> Arc<Registry> {
    Arc::new(
        Registry::new()
            .with_importer(StlImporter)
            .with_exporter(StlExporter::binary())
            .with_exporter(StlExporter::text()),
    )
}

pub(crate) fn read(bytes: impl Into<Arc<[u8]>>, name: &str) -> Model {
    Import::named_bytes(name, bytes, registry())
        .run()
        .expect("the STL file is read")
}

pub(crate) fn written(model: &Model, exporter: StlExporter) -> Vec<u8> {
    let exported = Export::with_exporter(model, Arc::new(exporter))
        .to_bytes("written.stl")
        .expect("the model is written");
    assert!(exported.warnings.is_empty(), "{:?}", exported.warnings);
    exported.bytes
}

pub(crate) fn triangles(model: &Model) -> Vec<[Vec3; 3]> {
    let world = model.world_transforms();
    let mut triangles = Vec::new();
    for (index, node) in model.nodes.iter().enumerate() {
        let Some(mesh) = node.mesh else {
            continue;
        };
        let mesh = model.mesh(mesh);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            triangles.push(triangle.map(|vertex| {
                world[index].transform_point3(mesh.vertices.positions[vertex as usize])
            }));
        }
    }
    triangles
}

pub(crate) fn area(model: &Model) -> f32 {
    triangles(model)
        .iter()
        .map(|[a, b, c]| (*b - *a).cross(*c - *a).length() / 2.0)
        .sum()
}

pub(crate) fn binary(header: &[u8], facets: &[([[f32; 3]; 4], u16)]) -> Vec<u8> {
    let mut bytes = header.to_vec();
    bytes.resize(80, b' ');
    bytes.extend_from_slice(&(facets.len() as u32).to_le_bytes());
    for (vectors, attribute) in facets {
        for value in vectors.as_flattened() {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&attribute.to_le_bytes());
    }
    bytes
}
