use std::time::Duration;

use crate::present::Acquired;
use crate::renderer::{FrameStart, Recorded, RenderOutput, Renderer};
use crate::scene::Scene;
use rikka_gpu::error::Result;

pub trait Present: Send + 'static {
    type Image: Send;

    fn start(&mut self, renderer: &mut Renderer, scene: &mut Scene) -> Result<FrameStart>;

    fn acquire_image(&mut self, timeout: Duration) -> Result<Acquired<Self::Image>>;

    fn record_image(
        &mut self,
        renderer: &mut Renderer,
        scene: &mut Scene,
        image: Self::Image,
    ) -> Result<Recorded>;

    fn present(&mut self, recorded: Recorded) -> Result<RenderOutput>;

    fn trim(&mut self) {}
}
