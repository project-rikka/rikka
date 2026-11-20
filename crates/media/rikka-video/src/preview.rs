use rikka_dmabuf_convert::{Converter, Imported, Preview};
use rikka_gpu::{DmabufSpec, Gpu};

use crate::error::Result;

pub struct Previewer {
    converter: Converter,
}

impl Previewer {
    pub fn new(gpu: &Gpu) -> Result<Previewer> {
        Ok(Previewer {
            converter: Converter::new(gpu.compute()?)?,
        })
    }

    pub fn import(&self, spec: &DmabufSpec<'_>) -> Result<Imported> {
        Ok(self.converter.import(spec)?)
    }

    pub fn preview(&self, source: &Imported, most: (u32, u32)) -> Result<Preview> {
        Ok(self.converter.preview(source, most)?)
    }
}

impl std::fmt::Debug for Previewer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Previewer").finish_non_exhaustive()
    }
}
