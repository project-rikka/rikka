use rikka_renderer::{Node, OffscreenTarget, Renderer, Scene};
use rikka_types::{Color, PixelSize, Point, Rect};

use crate::error::Result;
use crate::renderer::Three;
use crate::viewport::{Viewport, ViewportConfig};

pub struct OffscreenViewport {
    viewport: Viewport,
    target: OffscreenTarget,
    scene: Scene,
    size: PixelSize,
}

impl OffscreenViewport {
    pub fn open(renderer: &mut Renderer, config: ViewportConfig) -> Result<Self> {
        let size = PixelSize::new(config.width.max(1), config.height.max(1));
        let viewport = renderer.three().create_viewport(config)?;
        let target = renderer.create_target(size.width, size.height)?;
        let mut scene = Scene::new();
        scene.set_background(Color::BLACK);
        scene.insert(Node::image(
            Rect::from_origin_size(Point::ZERO, size.to_size()),
            viewport.texture(),
        ));
        Ok(Self {
            viewport,
            target,
            scene,
            size,
        })
    }

    #[must_use]
    pub fn size(&self) -> PixelSize {
        self.size
    }

    #[must_use]
    pub fn aspect(&self) -> f32 {
        self.size.aspect()
    }

    pub fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
    }

    pub fn present(&mut self, renderer: &mut Renderer) -> Result<&mut OffscreenTarget> {
        self.scene.notify_texture_updated(self.viewport.texture());
        renderer.draw(&mut self.scene, &mut self.target)?;
        Ok(&mut self.target)
    }

    pub fn read_pixels(&mut self, renderer: &mut Renderer) -> Result<Vec<u8>> {
        self.present(renderer)?;
        Ok(renderer.read_pixels(&mut self.target)?)
    }
}

impl std::fmt::Debug for OffscreenViewport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OffscreenViewport")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}
