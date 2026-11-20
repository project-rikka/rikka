use rikka_sync::Mutex;
use std::sync::Arc;

use rikka_renderer::{Renderer, TextureId};
use rikka_renderer_3d::{Frame, ViewportConfig};
use rikka_types::{Point, Rect};
use rikka_ui::Offscreen;

use crate::render::Render;

type SceneFn = dyn FnMut(SceneCtx<'_>) -> Option<Frame> + Send;

#[derive(Clone)]
pub struct Viewport3d {
    offscreen: Offscreen,
    shared: Arc<Shared>,
}

pub(crate) struct Shared {
    shape: Mutex<Shape>,
    scene: Mutex<Option<Box<SceneFn>>>,
}

struct Shape {
    config: ViewportConfig,
    config_revision: u64,
    pick_at: Option<(i32, i32)>,
    pick_result: Option<Option<u32>>,
}

#[derive(Debug)]
pub struct SceneCtx<'a> {
    pub renderer: &'a mut Renderer,
    pub width: u32,
    pub height: u32,
}

impl SceneCtx<'_> {
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }
}

impl Default for Viewport3d {
    fn default() -> Self {
        Self::new()
    }
}

impl Viewport3d {
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ViewportConfig::default())
    }

    #[must_use]
    pub fn with_config(config: ViewportConfig) -> Self {
        let shared = Arc::new(Shared {
            shape: Mutex::new(Shape {
                config,
                config_revision: 0,
                pick_at: None,
                pick_result: None,
            }),
            scene: Mutex::new(None),
        });
        let source = Arc::downgrade(&shared);
        Viewport3d {
            offscreen: Offscreen::new(move || Render::new(source.clone())),
            shared,
        }
    }

    pub fn set_scene(&self, scene: impl FnMut(SceneCtx<'_>) -> Option<Frame> + Send + 'static) {
        *self.shared.scene.lock() = Some(Box::new(scene));
    }

    #[must_use]
    pub fn config(&self) -> ViewportConfig {
        self.shared.config()
    }

    pub fn set_config(&self, config: ViewportConfig) {
        let mut shape = self.shared.shape.lock();
        shape.config = config;
        shape.config_revision += 1;
    }

    #[must_use]
    pub fn size(&self) -> Option<(u32, u32)> {
        self.offscreen.size()
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.offscreen.bounds()
    }

    pub fn pick(&self, at: Point) {
        let bounds = self.offscreen.bounds();
        let Some((width, height)) = self.offscreen.size() else {
            return;
        };
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return;
        }
        let x = (at.x - bounds.x()) / bounds.width() * width as f32;
        let y = (at.y - bounds.y()) / bounds.height() * height as f32;
        let mut shape = self.shared.shape.lock();
        if x < 0.0 || y < 0.0 || x >= width as f32 || y >= height as f32 {
            shape.pick_at = None;
            shape.pick_result = Some(None);
            return;
        }
        shape.pick_at = Some((x as i32, y as i32));
    }

    #[must_use]
    pub fn take_pick(&self) -> Option<Option<u32>> {
        self.shared.shape.lock().pick_result.take()
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.offscreen.id()
    }

    #[must_use]
    pub fn texture(&self) -> Option<TextureId> {
        self.offscreen.texture()
    }

    #[must_use]
    pub fn drawn_at(&self, bounds: Rect, width: u32, height: u32) -> Option<TextureId> {
        self.offscreen.drawn_at(bounds, width, height)
    }
}

impl std::fmt::Debug for Viewport3d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Viewport3d")
            .field("id", &self.id())
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}

impl Shared {
    pub(crate) fn config(&self) -> ViewportConfig {
        self.shape.lock().config
    }

    pub(crate) fn config_revision(&self) -> u64 {
        self.shape.lock().config_revision
    }

    pub(crate) fn take_pick_request(&self) -> Option<(i32, i32)> {
        self.shape.lock().pick_at.take()
    }

    pub(crate) fn answer_pick(&self, id: Option<u32>) {
        self.shape.lock().pick_result = Some(id);
    }

    pub(crate) fn build(&self, ctx: SceneCtx<'_>) -> Option<Frame> {
        self.scene.lock().as_mut().and_then(|scene| scene(ctx))
    }
}
