use crate::core::image::{GpuImage, ImageDesc, mip_levels_for};
use crate::core::runtime::BindPoint;
use crate::texture::book::Mips;
use crate::texture::{PICTURE_USAGE, TextureId};

use super::TexturePool;

pub(super) struct MipUpgrade {
    pub(super) id: TextureId,
    pub(super) former: GpuImage,
}

impl TexturePool {
    pub fn grow_minified(&mut self, bind: &mut BindPoint<'_>, retire_at: u64) -> bool {
        let mut grew = false;
        for id in self.book.take_minified() {
            match self.textures.contains_key(&id) {
                true => grew |= self.grow(bind, retire_at, id),
                false => self.book.defer_request(id),
            }
        }
        grew
    }

    pub fn grow(&mut self, bind: &mut BindPoint<'_>, retire_at: u64, id: TextureId) -> bool {
        if self
            .book
            .get(id)
            .is_none_or(|listing| listing.mips == Mips::Fixed)
        {
            return false;
        }
        let Some(entry) = self.textures.get_mut(&id) else {
            return false;
        };
        let extent = entry.image.extent();
        let desc = ImageDesc::color(extent, entry.image.format(), PICTURE_USAGE)
            .mips(mip_levels_for(extent))
            .components(entry.components)
            .shared_by(entry.shared_by);
        let created =
            GpuImage::create(bind.device.clone(), "rikka.texture", &desc).and_then(|image| {
                let slot = bind
                    .resources
                    .allocate(image.view(), bind.samplers.linear_clamp())?;
                Ok((image, slot))
            });
        let (image, slot) = match created {
            Ok(created) => created,
            Err(error) => {
                tracing::debug!("failed to create mip levels for {id:?}: {error}");
                self.book.settle_mips(id, None);
                return false;
            }
        };
        let former = std::mem::replace(&mut entry.image, image);
        bind.resources.release(entry.tex, retire_at);
        if self.streams.remove(&entry.tex).is_some() {
            self.streams.insert(slot, id);
        }
        entry.tex = slot;
        self.book.settle_mips(id, Some(slot));
        self.growing.push(MipUpgrade { id, former });
        true
    }
}
