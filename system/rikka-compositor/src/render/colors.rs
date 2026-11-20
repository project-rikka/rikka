use std::collections::HashMap;

use rikka_compositor_surface::SurfaceId;
use rikka_renderer::{ColorId, ImageColor, Renderer};

#[derive(Debug, Clone, Copy)]
struct Held {
    image: ImageColor,
    id: ColorId,
}

#[derive(Debug, Clone, Copy)]
enum Change {
    Retain(ColorId),
    Release(ColorId),
}

#[derive(Debug, Default)]
pub(crate) struct SurfaceColors {
    held: HashMap<SurfaceId, Held>,
    queued: Vec<Change>,
}

impl SurfaceColors {
    pub(crate) fn refresh(
        &mut self,
        surface: SurfaceId,
        image: ImageColor,
        renderer: &mut Renderer,
    ) -> bool {
        let previous = self.held.get(&surface).copied();
        if previous.is_some_and(|held| held.image == image) {
            return false;
        }
        let id = match renderer.colors().acquire(image) {
            Ok(id) => id,
            Err(error) => {
                tracing::warn!("cannot convert surface colours: {error}");
                ColorId::SRGB
            }
        };
        self.held.insert(surface, Held { image, id });
        if let Some(held) = previous {
            self.queued.push(Change::Release(held.id));
        }
        true
    }

    pub(crate) fn color(&self, surface: SurfaceId) -> ColorId {
        self.held
            .get(&surface)
            .map_or(ColorId::SRGB, |held| held.id)
    }

    pub(crate) fn forget(&mut self, surface: SurfaceId) {
        if let Some(held) = self.held.remove(&surface) {
            self.queued.push(Change::Release(held.id));
        }
    }

    pub(crate) fn keep(&mut self, id: ColorId) {
        self.queued.push(Change::Retain(id));
    }

    pub(crate) fn unkeep(&mut self, id: ColorId) {
        self.queued.push(Change::Release(id));
    }

    pub(crate) fn reclaim(&mut self, renderer: &mut Renderer) {
        let colors = renderer.colors();
        for change in self.queued.drain(..) {
            match change {
                Change::Retain(id) => colors.retain(id),
                Change::Release(id) => colors.release(id),
            }
        }
    }
}
