pub(crate) mod file;
pub(crate) mod read;

use rikka_model_core::{Confidence, ImportCtx, Importer, ImporterInfo, Input, Model, Result};

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
