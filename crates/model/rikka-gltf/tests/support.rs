use std::sync::Arc;

use rikka_gltf::{GltfExporter, GltfImporter};
use rikka_model_core::Registry;

pub(crate) fn registry() -> Arc<Registry> {
    Arc::new(
        Registry::new()
            .with_importer(GltfImporter)
            .with_exporter(GltfExporter::text())
            .with_exporter(GltfExporter::binary()),
    )
}
