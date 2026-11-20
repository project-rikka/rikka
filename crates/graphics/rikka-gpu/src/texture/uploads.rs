use std::sync::Arc;

use rikka_sync::Mutex;

use crate::compute::ComputedCopy;
use crate::core::device::RenderDevice;
use crate::core::resource_table::TextureSlots;
#[cfg(unix)]
use crate::drm::{DmabufSpec, Ycbcr};
use crate::error::{Error, Result};
use crate::reclaim::Reclaimer;
use crate::texture::packed::Packed;
use crate::texture::{
    Created, Kind, Pixels, RingSpan, Rows, Staged, TexelRect, TextureBook, TextureFormat,
    TextureId, UploadRing, prepared,
};
use crate::video::{ExternalVideoFrame, VideoDeviceHandles, VideoImports, video_handles};

#[derive(Clone)]
pub struct Uploads {
    queue: Arc<Queue>,
}

struct Queue {
    device: Arc<RenderDevice>,
    book: Arc<TextureBook>,
    slots: TextureSlots,
    reclaimer: Reclaimer,
    ring: Arc<UploadRing>,
    waiting: Mutex<Waiting>,
}

struct Waiting {
    open: bool,
    uploads: Vec<Upload>,
}

pub enum Upload {
    Insert {
        id: TextureId,
        slot: u32,
        created: Box<Created>,
    },
    Update {
        id: TextureId,
        offset: [u32; 2],
        extent: [u32; 2],
        pixels: Vec<u8>,
    },
    Staged {
        id: TextureId,
        rect: TexelRect,
        span: RingSpan,
    },
    Committed(TextureId),
    Destroy(TextureId),
    Video {
        texture: TextureId,
        frame: ExternalVideoFrame,
    },
    Computed {
        id: TextureId,
        offset: [u32; 2],
        copy: ComputedCopy,
    },
}

impl Upload {
    fn written(&self) -> Option<(TextureId, TexelRect)> {
        match self {
            Self::Update {
                id, offset, extent, ..
            } => Some((*id, TexelRect::new(*offset, *extent))),
            Self::Staged { id, rect, .. } => Some((*id, *rect)),
            _ => None,
        }
    }
}

impl Uploads {
    pub fn new(
        device: Arc<RenderDevice>,
        book: Arc<TextureBook>,
        slots: TextureSlots,
        reclaimer: Reclaimer,
        ring: Arc<UploadRing>,
    ) -> Self {
        Self {
            queue: Arc::new(Queue {
                device,
                book,
                slots,
                reclaimer,
                ring,
                waiting: Mutex::new(Waiting {
                    open: true,
                    uploads: Vec::new(),
                }),
            }),
        }
    }

    pub fn create(&self, pixels: Pixels) -> Result<TextureId> {
        let created = Created::picture(&self.queue.device, pixels)?;
        self.insert_created(created)
    }

    #[must_use]
    pub fn supports_drm_format(&self, fourcc: u32) -> bool {
        Packed::from_fourcc(fourcc).is_some_and(|packed| packed.sampled_on(&self.queue.device))
    }

    pub fn create_drm(&self, fourcc: u32, width: u32, height: u32) -> Result<TextureId> {
        let created = Created::drm(&self.queue.device, fourcc, width, height)?;
        self.insert_created(created)
    }

    pub fn stage(&self, id: TextureId, rows: Rows<'_>, rects: &[TexelRect]) -> Result<Staged> {
        let listing = self.queue.book.find(id)?;
        let texel_bytes = match (listing.kind, listing.texel_bytes) {
            (Kind::Picture, Some(bytes)) => bytes as usize,
            _ => {
                return Err(Error::Invalid(format!(
                    "texture {id:?} does not accept rows; use update_region"
                )));
            }
        };
        let mut parts = Vec::with_capacity(rects.len());
        for rect in rects {
            if !rect.fits(listing.extent) {
                return Err(Error::Invalid(format!(
                    "{}x{} rows at {},{} exceed a {}x{} texture",
                    rect.extent[0],
                    rect.extent[1],
                    rect.offset[0],
                    rect.offset[1],
                    listing.extent[0],
                    listing.extent[1]
                )));
            }
            let len = rect.extent[0] as usize * rect.extent[1] as usize * texel_bytes;
            let mut span = self.queue.ring.reserve(len)?;
            rows.copy_into(*rect, texel_bytes, span.bytes_mut())?;
            parts.push((*rect, span));
        }
        Ok(Staged {
            uploads: self.clone(),
            id,
            parts,
        })
    }

    pub fn blank(&self, width: u32, height: u32) -> Result<TextureId> {
        let created = Created::blank(&self.queue.device, width, height)?;
        self.insert_created(created)
    }

    #[cfg(unix)]
    pub fn import_dmabuf(&self, spec: &DmabufSpec<'_>) -> Result<TextureId> {
        self.import_ycbcr_dmabuf(spec, Ycbcr::guessed(spec.height))
    }

    #[cfg(unix)]
    pub fn import_ycbcr_dmabuf(&self, spec: &DmabufSpec<'_>, ycbcr: Ycbcr) -> Result<TextureId> {
        let created = Created::dmabuf(&self.queue.device, spec, ycbcr)?;
        tracing::trace!(
            target: "rikka_renderer::imports",
            width = spec.width,
            height = spec.height,
            "dmabuf imported"
        );
        self.insert_created(created)
    }

    pub fn create_compute_target(&self, width: u32, height: u32) -> Result<TextureId> {
        let created = Created::compute_target(&self.queue.device, width, height)?;
        self.insert_created(created)
    }

    pub fn copy_computed(&self, id: TextureId, offset: [u32; 2], copy: ComputedCopy) -> Result<()> {
        let listing = self.queue.book.find(id)?;
        if listing.kind != Kind::Picture {
            return Err(Error::Invalid(format!(
                "texture {id:?} does not accept computed images; create it with Uploads::create_compute_target"
            )));
        }
        let extent = copy.extent();
        let whole = listing.extent;
        if !TexelRect::new(offset, extent).fits(whole) {
            return Err(Error::Invalid(format!(
                "computed {}x{} region at {},{} exceeds a {}x{} texture",
                extent[0], extent[1], offset[0], offset[1], whole[0], whole[1]
            )));
        }
        self.send(Upload::Computed { id, offset, copy })
    }

    pub fn update(
        &self,
        id: TextureId,
        format: TextureFormat,
        data: impl Into<Vec<u8>>,
    ) -> Result<()> {
        let [width, height] = self.queue.book.find(id)?.extent;
        self.update_region(id, [0, 0], Pixels::new(width, height, format, data))
    }

    pub fn update_region(&self, id: TextureId, offset: [u32; 2], pixels: Pixels) -> Result<()> {
        let extent = pixels.extent;
        let listing = self.queue.book.find(id)?;
        if listing.kind == Kind::Imported {
            return Err(Error::Invalid(format!(
                "texture {id:?} is an imported buffer and is read-only"
            )));
        }
        let whole = listing.extent;
        if !TexelRect::new(offset, extent).fits(whole) {
            return Err(Error::Invalid(format!(
                "{}x{} upload at {},{} exceeds a {}x{} texture",
                extent[0], extent[1], offset[0], offset[1], whole[0], whole[1]
            )));
        }
        let texel = pixels.format.bytes_per_texel();
        if listing.texel_bytes.unwrap_or(4) != texel {
            return Err(Error::Invalid(format!(
                "{:?} upload does not match the texels of texture {id:?}",
                pixels.format
            )));
        }
        let pixels = prepared(pixels)?;
        self.send(Upload::Update {
            id,
            offset,
            extent,
            pixels,
        })
    }

    pub fn notify_committed(&self, id: TextureId) -> bool {
        let imported = self
            .queue
            .book
            .get(id)
            .is_some_and(|listing| listing.kind == Kind::Imported);
        imported && self.send(Upload::Committed(id)).is_ok()
    }

    pub fn destroy(&self, id: TextureId) -> bool {
        self.queue.book.remove(id).is_some() && self.send(Upload::Destroy(id)).is_ok()
    }

    pub unsafe fn submit_video_frame(
        &self,
        texture: TextureId,
        frame: ExternalVideoFrame,
    ) -> Result<()> {
        let listing = self.queue.book.find(texture)?;
        if listing.kind != Kind::Canvas {
            return Err(Error::Invalid(format!(
                "texture {texture:?} does not accept video frames; create it with Uploads::blank"
            )));
        }
        VideoImports::well_formed(&frame)?;
        self.send(Upload::Video { texture, frame })
    }

    #[must_use]
    pub fn extent(&self, id: TextureId) -> Option<[u32; 2]> {
        self.queue.book.get(id).map(|listing| listing.extent)
    }

    #[must_use]
    pub fn reclaimer(&self) -> Reclaimer {
        self.queue.reclaimer.clone()
    }

    #[must_use]
    pub fn video_device(&self) -> Option<VideoDeviceHandles> {
        video_handles(&self.queue.device)
    }

    pub fn take_into(&self, into: &mut Vec<Upload>) {
        into.append(&mut self.queue.waiting.lock().uploads);
    }

    pub fn close(&self) -> Vec<Upload> {
        let mut waiting = self.queue.waiting.lock();
        waiting.open = false;
        std::mem::take(&mut waiting.uploads)
    }

    pub fn trim_ring(&self) {
        self.queue.ring.trim();
    }

    pub(crate) fn send_staged(
        &self,
        id: TextureId,
        parts: Vec<(TexelRect, RingSpan)>,
    ) -> Result<()> {
        self.queue.book.find(id)?;
        let mut waiting = self.open()?;
        for (rect, span) in parts {
            queue_into(&mut waiting, Upload::Staged { id, rect, span });
        }
        Ok(())
    }

    fn insert_created(&self, created: Created) -> Result<TextureId> {
        let slot = self.queue.slots.reserve()?;
        let id = self.queue.book.allocate_id();
        self.queue.book.list(id, created.listing(slot));
        self.send(Upload::Insert {
            id,
            slot,
            created: Box::new(created),
        })?;
        Ok(id)
    }

    fn send(&self, upload: Upload) -> Result<()> {
        let mut waiting = self.open()?;
        queue_into(&mut waiting, upload);
        Ok(())
    }

    fn open(&self) -> Result<rikka_sync::MutexGuard<'_, Waiting>> {
        let waiting = self.queue.waiting.lock();
        if !waiting.open {
            return Err(Error::Invalid("renderer was dropped".into()));
        }
        Ok(waiting)
    }
}

fn queue_into(waiting: &mut Waiting, upload: Upload) {
    if let Some((id, rect)) = upload.written() {
        waiting
            .uploads
            .retain(|earlier| !overwrites(id, rect, earlier));
    }
    waiting.uploads.push(upload);
}

fn overwrites(id: TextureId, rect: TexelRect, earlier: &Upload) -> bool {
    let Some((under, covered)) = earlier.written() else {
        return false;
    };
    under == id
        && rect.offset[0] <= covered.offset[0]
        && rect.offset[1] <= covered.offset[1]
        && rect.offset[0] + rect.extent[0] >= covered.offset[0] + covered.extent[0]
        && rect.offset[1] + rect.extent[1] >= covered.offset[1] + covered.extent[1]
}

impl std::fmt::Debug for Uploads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Uploads")
            .field("waiting", &self.queue.waiting.lock().uploads.len())
            .finish_non_exhaustive()
    }
}

impl AsRef<Uploads> for Uploads {
    fn as_ref(&self) -> &Uploads {
        self
    }
}

impl std::fmt::Debug for Upload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Upload").finish_non_exhaustive()
    }
}
