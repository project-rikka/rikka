use std::path::Path;
use std::sync::{Arc, LazyLock};

use rikka_blend::BlendImporter;
use rikka_fbx::{FbxExporter, FbxImporter};
use rikka_gltf::{GltfExporter, GltfImporter};
use rikka_mmd::{PmxExporter, PmxImporter, VmdExporter, VmdImporter};
use rikka_stl::{StlExporter, StlImporter};
use rikka_vrm::{VrmAnimationImporter, VrmExporter, VrmImporter};

pub use rikka_humanoid as humanoid;
pub use rikka_mmd as mmd;
pub use rikka_model_core::*;
pub use rikka_vrm as vrm;

static SHIPPED: LazyLock<Arc<Registry>> = LazyLock::new(|| {
    Arc::new(
        Registry::new()
            .with_importer(GltfImporter)
            .with_importer(VrmImporter)
            .with_importer(VrmAnimationImporter)
            .with_importer(FbxImporter)
            .with_importer(PmxImporter)
            .with_importer(VmdImporter)
            .with_importer(StlImporter)
            .with_importer(BlendImporter)
            .with_exporter(GltfExporter::text())
            .with_exporter(GltfExporter::binary())
            .with_exporter(VrmExporter)
            .with_exporter(FbxExporter::binary())
            .with_exporter(FbxExporter::text())
            .with_exporter(PmxExporter)
            .with_exporter(StlExporter::binary())
            .with_exporter(StlExporter::text())
            .with_exporter(VmdExporter),
    )
});

#[must_use]
pub fn registry() -> Arc<Registry> {
    Arc::clone(&SHIPPED)
}

pub fn import(path: impl AsRef<Path>) -> Result<Model> {
    Import::open(path.as_ref(), registry()).run()
}
