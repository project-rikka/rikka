mod document;
mod glb;
mod read;
mod uri;
mod write;

pub use read::{Parts, SourceDocument, TextureMap, parts};
pub use write::{Composition, Container, Toon, compose, finish};

use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::{Confidence, ImportCtx, Importer, ImporterInfo, Input};
use rikka_model_core::{ExportCtx, Exporter, ExporterInfo, Supports};

const NAME: &str = "glTF";

const PARALLEL: rikka_parallel::Parallel = rikka_parallel::Parallel::new("rikka-gltf");

const BINARY_NAME: &str = "glTF Binary";

#[derive(Clone, Copy, Debug, Default)]
pub struct GltfImporter;

impl Importer for GltfImporter {
    fn info(&self) -> ImporterInfo {
        ImporterInfo {
            name: NAME,
            extensions: &["gltf", "glb"],
        }
    }

    fn sniff(&self, head: &[u8]) -> Confidence {
        if head.starts_with(&glb::MAGIC) {
            return Confidence::Yes;
        }
        let text = String::from_utf8_lossy(head);
        let json = text.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);
        match (json.starts_with('{'), json.contains("\"asset\"")) {
            (true, true) => Confidence::Yes,
            (true, false) => Confidence::Maybe,
            (false, _) => Confidence::No,
        }
    }

    fn read(&self, input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
        read::read(input, ctx)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GltfExporter {
    container: Container,
}

impl GltfExporter {
    #[must_use]
    pub fn text() -> Self {
        Self {
            container: Container::Text,
        }
    }

    #[must_use]
    pub fn binary() -> Self {
        Self {
            container: Container::Binary,
        }
    }
}

impl Exporter for GltfExporter {
    fn info(&self) -> ExporterInfo {
        ExporterInfo {
            name: match self.container {
                Container::Binary => BINARY_NAME,
                Container::Text => NAME,
            },
            extensions: match self.container {
                Container::Binary => &["glb"],
                Container::Text => &["gltf"],
            },
            supports: Supports::all(),
        }
    }

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
        write::write(model, ctx, self.container)
    }
}
