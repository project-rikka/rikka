use std::time::Instant;

use rikka_renderer::Scene;

use crate::motion::Clock;
use crate::picture::TextureResolver;

#[must_use]
pub struct Rendering<'a> {
    pub(super) scene: &'a mut Scene,
    pub(super) now: Option<Instant>,
    pub(super) shown: Option<Instant>,
    pub(super) resources: Option<&'a mut dyn TextureResolver>,
}

impl<'a> Rendering<'a> {
    pub fn new(scene: &'a mut Scene) -> Self {
        Self {
            scene,
            now: None,
            shown: None,
            resources: None,
        }
    }

    pub fn with_time(mut self, now: Instant) -> Self {
        self.now = Some(now);
        self
    }

    pub fn shown_at(mut self, shown: Instant) -> Self {
        self.shown = Some(shown);
        self
    }

    pub fn resolving(mut self, resources: &'a mut dyn TextureResolver) -> Self {
        self.resources = Some(resources);
        self
    }

    pub(super) fn clock(&self) -> Clock {
        let began = self.now.unwrap_or_else(Instant::now);
        Clock {
            now: self.shown.unwrap_or(began).max(began),
            began,
        }
    }
}

impl<'a> From<&'a mut Scene> for Rendering<'a> {
    fn from(scene: &'a mut Scene) -> Self {
        Self::new(scene)
    }
}

impl std::fmt::Debug for Rendering<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rendering")
            .field("now", &self.now)
            .field("shown", &self.shown)
            .field("resolving", &self.resources.is_some())
            .finish_non_exhaustive()
    }
}
