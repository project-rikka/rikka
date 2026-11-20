pub mod scene;

use std::path::Path;
use std::sync::{Arc, LazyLock};

use rikka_blend::{BlendExporter, BlendImporter};
use rikka_fbx::{FbxExporter, FbxImporter};
use rikka_gltf::{GltfExporter, GltfImporter};
use rikka_mmd::{PmxExporter, PmxImporter, VmdExporter, VmdImporter};
use rikka_stl::{StlExporter, StlImporter};
use rikka_vrm::{VrmAnimationImporter, VrmExporter, VrmImporter};

pub use rikka_blend::BlendOptions;
pub use rikka_humanoid as humanoid;
pub use rikka_mmd as mmd;
pub use rikka_model_core::*;
pub use rikka_vrm as vrm;

struct Shared(Arc<dyn Exporter>);

impl Exporter for Shared {
    fn info(&self) -> ExporterInfo {
        self.0.info()
    }

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
        self.0.write(model, ctx)
    }
}

static EXPORTERS: LazyLock<Vec<Arc<dyn Exporter>>> = LazyLock::new(|| {
    vec![
        Arc::new(GltfExporter::text()),
        Arc::new(GltfExporter::binary()),
        Arc::new(VrmExporter),
        Arc::new(FbxExporter::binary()),
        Arc::new(FbxExporter::text()),
        Arc::new(PmxExporter),
        Arc::new(StlExporter::binary()),
        Arc::new(StlExporter::text()),
        Arc::new(VmdExporter),
        Arc::new(BlendExporter::default()),
    ]
});

static SHIPPED: LazyLock<Arc<Registry>> = LazyLock::new(|| {
    let registry = Registry::new()
        .with_importer(GltfImporter)
        .with_importer(VrmImporter)
        .with_importer(VrmAnimationImporter)
        .with_importer(FbxImporter)
        .with_importer(PmxImporter)
        .with_importer(VmdImporter)
        .with_importer(StlImporter)
        .with_importer(BlendImporter);
    Arc::new(EXPORTERS.iter().fold(registry, |registry, exporter| {
        registry.with_exporter(Shared(Arc::clone(exporter)))
    }))
});

#[must_use]
pub fn registry() -> Arc<Registry> {
    Arc::clone(&SHIPPED)
}

#[must_use]
pub fn exporter_named(name: &str) -> Option<Arc<dyn Exporter>> {
    EXPORTERS
        .iter()
        .find(|exporter| exporter.info().name == name)
        .cloned()
}

pub fn import(path: impl AsRef<Path>) -> Result<Model> {
    Import::open(path.as_ref(), registry()).run()
}
