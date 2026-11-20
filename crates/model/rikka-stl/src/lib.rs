pub(crate) mod facet;
pub(crate) mod read;
pub(crate) mod write;

use rikka_model_core::CoordinateSystem;
use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::{Confidence, ImportCtx, Importer, ImporterInfo, Input};
use rikka_model_core::{ExportCtx, Exporter, ExporterInfo, Supports};

const NAME: &str = "STL";
const TEXT_NAME: &str = "STL Text";

pub(crate) const STL_UNIT_SCALE: f32 = 0.001;

pub(crate) const STL_COORDINATES: CoordinateSystem = CoordinateSystem::RIGHT_HANDED_Z_UP;

#[derive(Clone, Copy, Debug, Default)]
pub struct StlImporter;

impl Importer for StlImporter {
    fn info(&self) -> ImporterInfo {
        ImporterInfo {
            name: NAME,
            extensions: &["stl"],
        }
    }

    fn sniff(&self, head: &[u8]) -> Confidence {
        read::sniff(head)
    }

    fn read(&self, input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
        read::read(input, ctx)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StlExporter {
    text: bool,
}

impl StlExporter {
    #[must_use]
    pub fn binary() -> Self {
        Self { text: false }
    }

    #[must_use]
    pub fn text() -> Self {
        Self { text: true }
    }
}

impl Exporter for StlExporter {
    fn info(&self) -> ExporterInfo {
        ExporterInfo {
            name: match self.text {
                true => TEXT_NAME,
                false => NAME,
            },
            extensions: &["stl"],
            supports: Supports::none(),
        }
    }

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
        write::write(model, ctx, self.text)
    }
}
