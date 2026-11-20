use rikka_sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use rikka_renderer::{Renderer, TextureId};
use rikka_types::Rect;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

static LIVE: Mutex<Vec<Weak<Shared>>> = Mutex::new(Vec::new());

pub trait OffscreenRender: Send {
    fn render(&mut self, renderer: &mut Renderer, width: u32, height: u32) -> Option<TextureId>;
}

type Source = dyn Fn() -> Box<dyn OffscreenRender> + Send + Sync;

#[derive(Clone)]
pub struct Offscreen(Arc<Shared>);

pub trait OffscreenConsumer {
    fn set_texture(&self, texture: TextureId) -> bool;

    fn renderer(&self) -> Box<dyn OffscreenRender>;
}

struct Shared {
    id: u64,
    source: Box<Source>,
    shape: Mutex<Shape>,
}

struct Shape {
    requested_size: Option<(u32, u32)>,
    texture: Option<TextureId>,
    bounds: Rect,
}

impl Offscreen {
    #[must_use]
    pub fn new<R: OffscreenRender + 'static>(
        source: impl Fn() -> R + Send + Sync + 'static,
    ) -> Self {
        let shared = Arc::new(Shared {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            source: Box::new(move || Box::new(source())),
            shape: Mutex::new(Shape {
                requested_size: None,
                texture: None,
                bounds: Rect::ZERO,
            }),
        });
        LIVE.lock().push(Arc::downgrade(&shared));
        Offscreen(shared)
    }

    #[must_use]
    pub fn size(&self) -> Option<(u32, u32)> {
        self.0.shape.lock().requested_size
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.0.shape.lock().bounds
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.0.id
    }

    #[must_use]
    pub fn texture(&self) -> Option<TextureId> {
        self.0.shape.lock().texture
    }

    #[must_use]
    pub fn drawn_at(&self, bounds: Rect, width: u32, height: u32) -> Option<TextureId> {
        let mut shape = self.0.shape.lock();
        shape.requested_size = Some((width, height));
        shape.bounds = bounds;
        shape.texture
    }
}

impl std::fmt::Debug for Offscreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Offscreen")
            .field("id", &self.0.id)
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}

impl OffscreenConsumer for Offscreen {
    fn set_texture(&self, texture: TextureId) -> bool {
        self.0.shape.lock().texture.replace(texture) != Some(texture)
    }

    fn renderer(&self) -> Box<dyn OffscreenRender> {
        (self.0.source)()
    }
}

#[must_use]
pub fn live_offscreens() -> Vec<Offscreen> {
    let mut live = LIVE.lock();
    live.retain(|handle| handle.strong_count() > 0);
    live.iter()
        .filter_map(Weak::upgrade)
        .map(Offscreen)
        .collect()
}

#[must_use]
pub fn any_offscreen_live() -> bool {
    LIVE.lock().iter().any(|handle| handle.strong_count() > 0)
}
