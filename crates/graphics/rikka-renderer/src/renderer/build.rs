use std::sync::Arc;

use crate::renderer::history::Targets;
use crate::renderer::painter::{Painter, Sharing};
use crate::renderer::sources::Sources;
use crate::renderer::{Colors, ExtensionSlot, Renderer};
use rikka_gpu::Gpu;
use rikka_gpu::core::runtime::GpuRuntime;
use rikka_gpu::core::transfer::TransferLane;
use rikka_gpu::error::Result;
use rikka_gpu::reclaim::Reclaimer;
use rikka_gpu::texture::{TextureBook, UploadRing, Uploads};

impl Renderer {
    pub fn new(gpu: &Gpu) -> Result<Renderer> {
        let device = gpu.device().clone();
        let runtime = GpuRuntime::new(device.clone())?;
        let book = Arc::new(TextureBook::default());
        let reclaimer = Reclaimer::default();
        let painter = Painter::new(
            device.clone(),
            runtime.table_layout(),
            Sharing {
                book: Arc::clone(&book),
                slots: runtime.texture_slots(),
                owner: reclaimer.clone(),
                timeline: Arc::clone(runtime.timeline()),
            },
        )?;
        let transfer = TransferLane::new(device.clone())?;
        let ring = UploadRing::new(
            device.clone(),
            Arc::clone(runtime.timeline()),
            transfer.as_ref().map(|lane| Arc::clone(lane.timeline())),
        );
        let uploads = Uploads::new(
            device.clone(),
            Arc::clone(&book),
            runtime.texture_slots(),
            reclaimer.clone(),
            ring,
        );

        Ok(Renderer {
            painter,
            sources: Sources::new(device, runtime.table_layout(), book, transfer),
            targets: Targets::default(),
            colors: Colors::default(),
            extension: ExtensionSlot::default(),
            reclaimer,
            reclaimed: Vec::new(),
            uploads,
            taken_uploads: Vec::new(),
            device: gpu.clone(),
            gpu: runtime,
        })
    }
}
