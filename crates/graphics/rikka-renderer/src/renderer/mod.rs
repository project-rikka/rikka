mod build;
mod canvas;
mod colors;
mod copy_out;
mod destination;
mod device;
#[cfg(unix)]
mod dmabuf;
mod draw;
mod extension;
mod history;
mod painter;
mod preparer;
mod readback;
mod sources;
#[cfg(windows)]
mod swapchain;
mod textures;
mod work;

use std::time::{Duration, Instant};

use crate::draw::retained::Reclaimed;
use rikka_gpu::Gpu;
use rikka_gpu::core::runtime::{GpuRuntime, UNUSED_FOR};
use rikka_gpu::reclaim::{ReclaimRequest, Reclaimer};
use rikka_gpu::texture::{TextureId, Upload, Uploads};

use history::Targets;
use painter::Painter;
use sources::Sources;

pub use colors::{AlphaMode, ColorId, Colors, ImageColor};
pub use copy_out::{CopiedDraw, CopySink};
#[cfg(unix)]
pub use dmabuf::{DmabufDraw, DmabufFence, DmabufFrame, DmabufOutput};
pub use draw::{Damage, Destination, FrameStart, Recorded, RenderOutput};
pub use extension::{Extension, ExtensionSlot, RendererParts};
pub use history::DamageSource;
pub use preparer::Preparer;
pub use readback::Readback;
#[cfg(windows)]
pub use swapchain::{SwapchainDraw, SwapchainFrame, SwapchainOutput};
pub use textures::Textures;
pub use work::FrameWork;

pub const TRIM_AFTER: Duration = Duration::from_secs(UNUSED_FOR.as_secs() + 1);
pub struct Renderer {
    painter: Painter,
    sources: Sources,
    targets: Targets,
    colors: Colors,
    extension: ExtensionSlot,
    reclaimer: Reclaimer,
    reclaimed: Vec<ReclaimRequest>,
    uploads: Uploads,
    taken_uploads: Vec<Upload>,
    device: Gpu,
    gpu: GpuRuntime,
}

impl Renderer {
    #[must_use]
    pub fn reclaimer(&self) -> Reclaimer {
        self.reclaimer.clone()
    }

    pub fn retain_backdrops(&mut self, keys: &[u64]) {
        self.painter.fx.retain_backdrops(keys);
    }

    pub fn reclaim(&mut self) {
        self.take_uploads();
        let mut left = std::mem::take(&mut self.reclaimed);
        self.reclaimer.take_into(&mut left);
        for item in left.drain(..) {
            self.reclaim_one(item);
        }
        self.reclaimed = left;
    }

    pub fn trim(&mut self) {
        self.reclaim();
        let now = Instant::now();
        let mut recycler = self.gpu.idle_recycler();
        self.painter.trim(&mut recycler, now);
        self.targets.trim(&mut recycler, now);
        self.gpu.trim();
        self.sources.trim();
        self.uploads.trim_ring();
        self.device.trim();
    }

    fn reclaim_one(&mut self, item: ReclaimRequest) {
        match item {
            ReclaimRequest::Texture(id) => {
                self.destroy_texture(id);
            }
            ReclaimRequest::Target(id) => self.targets.retire(&mut self.gpu, id),
            ReclaimRequest::Buffer(id) => self.targets.forget_buffer(id),
            ReclaimRequest::Gpu(resource) => self.gpu.defer(resource),
            ReclaimRequest::Routed(request) => match request.downcast::<Reclaimed>() {
                Ok(request) => self.reclaim_canvas(*request),
                Err(request) => {
                    self.extension
                        .reclaim(request, &mut self.gpu, &mut self.sources.textures);
                }
            },
        }
    }

    fn reclaim_canvas(&mut self, request: Reclaimed) {
        match request {
            Reclaimed::Glyphs(keys) => self.painter.release_glyphs(&keys),
            Reclaimed::Instances(scene) => {
                if let Some(store) = self.painter.forget_scene(scene) {
                    self.gpu.defer(Box::new(store));
                }
            }
        }
    }

    pub(crate) fn destroy_texture(&mut self, id: TextureId) -> bool {
        self.sources.destroy_texture(&mut self.gpu.recycler(), id)
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.uploads.close();
        if self.gpu.wait_all().is_err() {
            self.gpu.device().wait_idle();
        }
        self.reclaimer.take_into(&mut self.reclaimed);
        self.reclaimed.clear();
    }
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer").finish_non_exhaustive()
    }
}
