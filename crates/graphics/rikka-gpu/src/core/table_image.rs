use ash::vk;

use crate::core::image::GpuImage;
use crate::core::resource_table::{ResourceTable, TextureSlots};
use crate::core::runtime::Recycler;
use crate::error::Result;

pub struct TableImage {
    pub image: GpuImage,
    pub tex: u32,
}

impl TableImage {
    pub fn register(
        image: GpuImage,
        resources: &mut ResourceTable,
        sampler: vk::Sampler,
    ) -> Result<Self> {
        let tex = resources.allocate(image.view(), sampler)?;
        Ok(Self { image, tex })
    }

    pub fn reserve(image: GpuImage, slots: &TextureSlots) -> Result<Self> {
        let tex = slots.reserve()?;
        Ok(Self { image, tex })
    }

    pub fn bind(&self, resources: &mut ResourceTable, sampler: vk::Sampler) {
        resources.bind_reserved(self.tex, self.image.view(), sampler);
    }

    pub fn retire(self, recycler: &mut Recycler<'_>) {
        recycler.resources.release(self.tex, recycler.value);
        recycler.retire.defer(recycler.value, Box::new(self.image));
    }
}

impl std::fmt::Debug for TableImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableImage").finish_non_exhaustive()
    }
}
