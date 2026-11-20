mod mips;
mod record;
mod transfer;

use std::sync::Arc;

use ash::vk;
use rustc_hash::FxHashMap;

use crate::compute::ComputedCopy;
use crate::core::image::GpuImage;
use crate::core::runtime::{BindPoint, Recycler};
use crate::core::transfer::DeferredDrop;
use crate::error::Result;
use crate::texture::book::{Kind, Listing, Mips, TextureBook};
use crate::texture::created::Created;
use crate::texture::entry::TextureEntry;
use crate::texture::pending_upload::{Fill, PendingUpload};
use crate::texture::ring::RingSpan;
use crate::texture::rows::TexelRect;
use crate::texture::source::TextureSource;
use crate::texture::usage::Use;
use crate::texture::{OUTPUT_FORMAT, TextureId};

use mips::MipUpgrade;

pub struct TexturePool {
    book: Arc<TextureBook>,
    textures: FxHashMap<TextureId, TextureEntry>,
    foreign: FxHashMap<u32, TextureId>,
    streams: FxHashMap<u32, TextureId>,
    deferred: Option<DeferredDrop>,
    taken: Vec<TextureId>,
    pending: Vec<PendingUpload>,
    committed: Vec<TextureId>,
    sent: Vec<PendingUpload>,
    converted: Vec<TextureId>,
    growing: Vec<MipUpgrade>,
    sent_grown: Vec<MipUpgrade>,
    streaming: Vec<TextureId>,
    barriers: Vec<vk::ImageMemoryBarrier2<'static>>,
}

impl TexturePool {
    pub fn new(book: Arc<TextureBook>, deferred: Option<DeferredDrop>) -> Self {
        Self {
            book,
            textures: FxHashMap::default(),
            foreign: FxHashMap::default(),
            streams: FxHashMap::default(),
            deferred,
            taken: Vec::new(),
            pending: Vec::new(),
            committed: Vec::new(),
            sent: Vec::new(),
            converted: Vec::new(),
            growing: Vec::new(),
            sent_grown: Vec::new(),
            streaming: Vec::new(),
            barriers: Vec::new(),
        }
    }

    pub fn insert_created(
        &mut self,
        bind: &mut BindPoint<'_>,
        id: TextureId,
        slot: u32,
        created: Created,
    ) {
        let listing = created.listing(slot);
        let Created {
            image,
            source,
            fill,
            components,
            shared_by,
            ..
        } = created;
        bind.resources
            .bind_reserved(slot, image.view(), bind.samplers.linear_clamp());
        self.enter(id, image, source, listing);
        if let Some(entry) = self.textures.get_mut(&id) {
            entry.components = components;
            entry.shared_by = shared_by;
        }
        if shared_by.is_some() {
            self.streams.insert(slot, id);
        }
        if let Some(fill) = fill {
            self.pending.push(PendingUpload {
                id,
                offset: [0, 0],
                extent: listing.extent,
                fill,
            });
        }
    }

    pub fn mark_imported_dirty(&mut self, id: TextureId) -> bool {
        match self.textures.get_mut(&id) {
            Some(entry) if entry.source.is_imported() => {
                if !entry.committed {
                    entry.committed = true;
                    self.committed.push(id);
                }
                true
            }
            _ => false,
        }
    }

    pub fn update_computed(&mut self, id: TextureId, offset: [u32; 2], copy: ComputedCopy) {
        if !self.textures.contains_key(&id) {
            return;
        }
        let upload = PendingUpload {
            id,
            offset,
            extent: copy.extent(),
            fill: Fill::Computed(Box::new(copy)),
        };
        self.pending
            .retain(|pending| pending.id != id || !upload.covers(pending));
        self.pending.push(upload);
    }

    pub fn update(&mut self, id: TextureId, offset: [u32; 2], extent: [u32; 2], pixels: Vec<u8>) {
        self.queue(PendingUpload {
            id,
            offset,
            extent,
            fill: Fill::Pixels(pixels),
        });
    }

    pub fn update_staged(&mut self, id: TextureId, rect: TexelRect, span: RingSpan) {
        self.queue(PendingUpload {
            id,
            offset: rect.offset,
            extent: rect.extent,
            fill: Fill::Staged(span),
        });
    }

    fn queue(&mut self, upload: PendingUpload) {
        if !self.textures.contains_key(&upload.id) {
            return;
        }
        self.pending
            .retain(|pending| pending.id != upload.id || !upload.covers(pending));
        self.pending.push(upload);
    }

    pub fn destroy(&mut self, recycler: &mut Recycler<'_>, id: TextureId) -> bool {
        self.pending.retain(|pending| pending.id != id);
        self.committed.retain(|committed| *committed != id);
        self.taken.retain(|taken| *taken != id);
        self.book.remove(id);
        let arrival = self.textures.get(&id).map_or(0, |entry| entry.arrival);
        let (gone, growing) = std::mem::take(&mut self.growing)
            .into_iter()
            .partition(|grown| grown.id == id);
        self.growing = growing;
        for grown in gone {
            let former = self.drop_after(arrival, Box::new(grown.former));
            recycler.retire.defer(recycler.value, former);
        }
        if let Some(entry) = self.textures.remove(&id) {
            self.foreign.remove(&entry.tex);
            self.streams.remove(&entry.tex);
            recycler.resources.release(entry.tex, recycler.value);
            let image = self.drop_after(arrival, Box::new(entry.image));
            recycler.retire.defer(recycler.value, image);
            #[cfg(unix)]
            if let TextureSource::Planar(planes) = entry.source {
                recycler.retire.defer(recycler.value, Box::new(planes));
            }
            return true;
        }
        false
    }

    fn drop_after(
        &self,
        arrival: u64,
        item: Box<dyn std::any::Any + Send>,
    ) -> Box<dyn std::any::Any + Send> {
        match &self.deferred {
            Some(deferred) if !deferred.timeline().reached(arrival) => {
                deferred.drop_after(arrival, item)
            }
            _ => item,
        }
    }

    pub fn slot(&self, id: TextureId) -> Option<u32> {
        self.textures.get(&id).map(|entry| entry.tex)
    }

    pub fn image(&self, id: TextureId) -> Option<&GpuImage> {
        self.textures.get(&id).map(|entry| &entry.image)
    }

    pub fn image_mut(&mut self, id: TextureId) -> Option<&mut GpuImage> {
        self.textures.get_mut(&id).map(|entry| &mut entry.image)
    }

    pub fn takes_frames(&self, id: TextureId) -> bool {
        self.textures.contains_key(&id)
            && self
                .book
                .get(id)
                .is_some_and(|listing| listing.kind == Kind::Canvas)
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.committed.is_empty() || !self.growing.is_empty()
    }

    #[must_use]
    pub fn commit(&mut self) -> Vec<GpuImage> {
        self.sent.clear();
        self.converted.clear();
        self.sent_grown
            .drain(..)
            .map(|grown| grown.former)
            .collect()
    }

    pub fn revert(&mut self) {
        let later = std::mem::take(&mut self.pending);
        self.pending = std::mem::take(&mut self.sent);
        self.pending.extend(later);
        let later = std::mem::take(&mut self.growing);
        self.growing = std::mem::take(&mut self.sent_grown);
        self.growing.extend(later);
        for id in std::mem::take(&mut self.converted) {
            if let Some(entry) = self.textures.get_mut(&id)
                && !entry.committed
            {
                entry.committed = true;
                self.committed.push(id);
            }
        }
    }

    pub fn create_target(
        &mut self,
        bind: &mut BindPoint<'_>,
        width: u32,
        height: u32,
    ) -> Result<TextureId> {
        let image =
            Use::Canvas.image(bind.device, vk::Extent2D { width, height }, OUTPUT_FORMAT)?;
        let slot = bind
            .resources
            .allocate(image.view(), bind.samplers.linear_clamp())?;
        let id = self.book.allocate_id();
        let listing = Listing {
            slot,
            extent: [width, height],
            kind: Kind::Canvas,
            mips: Mips::Fixed,
            texel_bytes: None,
        };
        self.enter(id, image, TextureSource::Memory, listing);
        Ok(id)
    }

    fn enter(&mut self, id: TextureId, image: GpuImage, source: TextureSource, listing: Listing) {
        let committed = source.is_imported();
        #[cfg(unix)]
        if matches!(source, TextureSource::Dmabuf) {
            self.foreign.insert(listing.slot, id);
        }
        self.book.list(id, listing);
        self.textures.insert(
            id,
            TextureEntry {
                image,
                tex: listing.slot,
                source,
                committed,
                components: vk::ComponentMapping::default(),
                shared_by: None,
                arrival: 0,
            },
        );
        if committed {
            self.committed.push(id);
        }
    }
}

impl std::fmt::Debug for TexturePool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TexturePool").finish_non_exhaustive()
    }
}
