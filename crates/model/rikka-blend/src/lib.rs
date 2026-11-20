mod export;
pub(crate) mod file;
pub(crate) mod read;
mod scene;
pub(crate) mod write;

use rikka_model_core::{
    Confidence, Exported, ImportCtx, ImportOptions, Importer, ImporterInfo, Input, Model, Resolver,
    Result,
};
use rikka_model_scene::Scene;
use rikka_types::CancelToken;

pub use export::BlendExporter;
pub use scene::{BlendOptions, BlendScene};

const NAME: &str = "Blender";

#[derive(Clone, Copy, Debug, Default)]
pub struct BlendImporter;

impl Importer for BlendImporter {
    fn info(&self) -> ImporterInfo {
        ImporterInfo {
            name: NAME,
            extensions: &["blend"],
        }
    }

    fn sniff(&self, head: &[u8]) -> Confidence {
        sniff(head)
    }

    fn read(&self, input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
        read::read(input, ctx)
    }
}

#[must_use]
pub fn sniff(head: &[u8]) -> Confidence {
    if head.starts_with(file::MAGIC) {
        return Confidence::Yes;
    }
    match file::packing(head) {
        file::Packing::Plain => Confidence::No,
        _ => match file::peek(head, file::MAGIC.len()) {
            Some(peeked) if peeked == file::MAGIC => Confidence::Yes,
            Some(_) => Confidence::No,
            None => Confidence::Maybe,
        },
    }
}

pub fn read_scene(
    input: &Input<'_>,
    resolver: &dyn Resolver,
    options: &ImportOptions,
    cancel: &CancelToken,
) -> Result<BlendScene> {
    scene::read(input, resolver, options, cancel)
}

pub fn write_scene(scene: &Scene, options: &BlendOptions) -> Result<Exported> {
    scene::write(scene, options)
}
