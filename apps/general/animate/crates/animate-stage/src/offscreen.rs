use rikka_renderer::{Node, OffscreenTarget, Renderer, Scene};
use rikka_renderer_3d::{Three, Viewport, ViewportConfig};
use rikka_types::{Color, PixelSize, Point, Rect};

pub(super) struct Offscreen {
    viewport: Viewport,
    target: OffscreenTarget,
    scene: Scene,
    aspect: f32,
}

impl Offscreen {
    pub(super) fn open(
        renderer: &mut Renderer,
        size: PixelSize,
    ) -> rikka_renderer_3d::Result<Self> {
        let PixelSize { width, height } = size;
        let viewport = renderer
            .three()
            .create_viewport(ViewportConfig::new(width, height).identity(false))?;
        let target = renderer.create_target(width, height)?;
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
            aspect: size.aspect(),
        })
    }

    pub(super) fn aspect(&self) -> f32 {
        self.aspect
    }

    pub(super) fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
    }

    pub(super) fn present(
        &mut self,
        renderer: &mut Renderer,
    ) -> rikka_renderer_3d::Result<&mut OffscreenTarget> {
        self.scene.notify_texture_updated(self.viewport.texture());
        renderer.draw(&mut self.scene, &mut self.target)?;
        Ok(&mut self.target)
    }
}

impl std::fmt::Debug for Offscreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Offscreen")
            .field("aspect", &self.aspect)
            .finish_non_exhaustive()
    }
}
