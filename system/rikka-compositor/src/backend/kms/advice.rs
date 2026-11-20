use rikka_compositor_kms::Gpu;
use rikka_compositor_surface::DmabufFeedback;
use rikka_compositor_types::FormatSet;
use rikka_renderer::Renderer;
use rustix::fs::Dev;

use crate::wayland::{Tranches, primary_node};

#[derive(Debug, Clone)]
pub(super) struct Advice {
    tranches: Option<Tranches>,
    device: Dev,
}

impl Advice {
    pub(super) fn new(renderer: &Renderer, gpu: &Gpu) -> Self {
        Self {
            tranches: Tranches::from_renderer(renderer),
            device: primary_node(gpu.major(), gpu.minor()),
        }
    }

    pub(super) fn scanout(&self, plane: &FormatSet) -> Option<DmabufFeedback> {
        self.tranches.as_ref()?.scanned_out(self.device, plane)
    }
}
