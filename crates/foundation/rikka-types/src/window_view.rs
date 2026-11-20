use std::sync::Arc;

use crate::{Rect, RoundedRect};

#[derive(Clone, Debug, PartialEq)]
pub struct WindowView {
    pub window: Arc<str>,
    pub rect: Rect,
    pub clip: Option<RoundedRect>,
    pub opacity: f32,
}

impl WindowView {
    #[must_use]
    pub fn new(window: impl Into<Arc<str>>, rect: Rect) -> Self {
        Self {
            window: window.into(),
            rect,
            clip: None,
            opacity: 1.0,
        }
    }

    #[must_use]
    pub fn clip(mut self, clip: impl Into<Option<RoundedRect>>) -> Self {
        self.clip = clip.into();
        self
    }

    #[must_use]
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    #[must_use]
    pub fn scaled(mut self, by: f32) -> Self {
        self.rect = self.rect.scale(by);
        self.clip = self.clip.map(|clip| clip.scale(by));
        self
    }

    #[must_use]
    pub fn translated(mut self, by: crate::Point) -> Self {
        self.rect = self.rect.translate(by);
        if let Some(clip) = &mut self.clip {
            clip.rect = clip.rect.translate(by);
        }
        self
    }
}
