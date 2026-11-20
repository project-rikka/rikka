use std::sync::Arc;

use rikka_fbx::{FbxExporter, FbxImporter};
use rikka_model_core::Registry;

pub(crate) const KNIGHT: &[u8] = include_bytes!("../assets/knight.fbx");

pub(crate) fn registry() -> Arc<Registry> {
    Arc::new(
        Registry::new()
            .with_importer(FbxImporter)
            .with_exporter(FbxExporter::binary())
            .with_exporter(FbxExporter::text()),
    )
}
