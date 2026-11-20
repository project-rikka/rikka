use std::sync::Arc;

use rikka_gltf::{GltfExporter, GltfImporter};
use rikka_model_core::{Export, ExportOptions, Exported, Exporter, Import, Model, Registry};
use rikka_vrm::{Vrm, VrmExporter, VrmImporter};

pub(crate) fn registry() -> Arc<Registry> {
    Arc::new(
        Registry::new()
            .with_importer(VrmImporter)
            .with_importer(GltfImporter)
            .with_exporter(VrmExporter)
            .with_exporter(GltfExporter::text()),
    )
}

pub(crate) fn vrm(model: &Model) -> &Vrm {
    model.extensions.get::<Vrm>().expect("a VRM")
}

pub(crate) fn export(model: &Model, exporter: impl Exporter) -> Exported {
    Export::with_exporter(model, Arc::new(exporter))
        .options(ExportOptions::new().embed_resources(true))
        .to_bytes("avatar.vrm")
        .expect("the avatar is written")
}

pub(crate) fn import(bytes: Vec<u8>) -> Model {
    Import::named_bytes("avatar.vrm", bytes, registry())
        .run()
        .expect("the written avatar is read")
}

pub(crate) fn document(glb: &[u8]) -> serde_json::Value {
    let length = u32::from_le_bytes(glb[12..16].try_into().expect("a chunk length")) as usize;
    serde_json::from_slice(&glb[20..20 + length]).expect("a JSON chunk")
}
