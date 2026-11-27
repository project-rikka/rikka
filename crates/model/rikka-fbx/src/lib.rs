pub(crate) mod document;
pub(crate) mod euler;
pub(crate) mod read;
pub(crate) mod write;

use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::{Confidence, ImportCtx, Importer, ImporterInfo, Input};
use rikka_model_core::{ExportCtx, Exporter, ExporterInfo, Supports};

const NAME: &str = "FBX";
const TEXT_NAME: &str = "FBX Text";
const PARALLEL: rikka_parallel::Parallel = rikka_parallel::Parallel::new("rikka-fbx");

pub(crate) const KTIME: i64 = 46_186_158_000;

pub(crate) const KTIME_LATER: i64 = 141_120_000;

#[derive(Clone, Copy, Debug, Default)]
pub struct FbxImporter;

impl Importer for FbxImporter {
    fn info(&self) -> ImporterInfo {
        ImporterInfo {
            name: NAME,
            extensions: &["fbx"],
        }
    }

    fn sniff(&self, head: &[u8]) -> Confidence {
        if head.starts_with(read::binary::MAGIC) {
            return Confidence::Yes;
        }
        let text = String::from_utf8_lossy(head);
        match text.contains("FBXHeaderExtension") {
            true => Confidence::Yes,
            false => match text.contains("FBX") && text.contains("project file") {
                true => Confidence::Maybe,
                false => Confidence::No,
            },
        }
    }

    fn read(&self, input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
        read::read(input, ctx)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FbxExporter {
    text: bool,
}

impl FbxExporter {
    #[must_use]
    pub fn binary() -> Self {
        Self { text: false }
    }

    #[must_use]
    pub fn text() -> Self {
        Self { text: true }
    }
}

impl Exporter for FbxExporter {
    fn info(&self) -> ExporterInfo {
        ExporterInfo {
            name: match self.text {
                true => TEXT_NAME,
                false => NAME,
            },
            extensions: &["fbx"],
            supports: Supports::all(),
        }
    }

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
        write::write(model, ctx, self.text)
    }
}
