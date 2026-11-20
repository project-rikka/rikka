mod image;

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::{CursorImage as Shown, CursorShape};
use rikka_compositor_surface::SurfaceId;
use rikka_xcursor::Theme;

pub(crate) use image::CursorImage;

const DEFAULT_SIZE: u32 = 24;

pub(crate) struct Cursor {
    position: Point<f64, Logical>,
    shown: Shown,
    decor: Option<CursorShape>,
    theme: Theme,
    named: String,
    size: u32,
    images: HashMap<(CursorShape, u32), Arc<CursorImage>>,
    fallback: Arc<CursorImage>,
}

impl Cursor {
    pub(crate) fn new() -> Self {
        let named = machine_theme();
        Self {
            position: Point::zero(),
            shown: Shown::default(),
            decor: None,
            theme: Theme::load(&named),
            named,
            size: std::env::var("XCURSOR_SIZE")
                .ok()
                .and_then(|size| size.parse().ok())
                .unwrap_or(DEFAULT_SIZE),
            images: HashMap::new(),
            fallback: image::fallback(),
        }
    }

    pub(crate) fn set_theme(&mut self, theme: Option<&str>, size: u32) -> bool {
        let named = theme.map_or_else(machine_theme, str::to_owned);
        if named == self.named && size == self.size {
            return false;
        }
        if named != self.named {
            self.theme = Theme::load(&named);
            self.named = named;
        }
        self.size = size;
        self.images.clear();
        true
    }

    pub(crate) fn theme_and_size(&self) -> (&str, u32) {
        (&self.named, self.size)
    }

    pub(crate) fn show(&mut self, shown: Shown) -> bool {
        std::mem::replace(&mut self.shown, shown) != shown
    }

    pub(crate) fn decorate(&mut self, decor: Option<CursorShape>) -> bool {
        std::mem::replace(&mut self.decor, decor) != decor
    }

    pub(crate) fn hidden(&self) -> bool {
        self.decor.is_none() && self.shown == Shown::Hidden
    }

    pub(crate) fn surface(&self) -> Option<(SurfaceId, Point<i32, Logical>)> {
        match (self.decor, self.shown) {
            (None, Shown::Surface { surface, hotspot }) => Some((surface, hotspot)),
            _ => None,
        }
    }

    pub(crate) fn forget_surface(&mut self, surface: SurfaceId) -> bool {
        if matches!(self.shown, Shown::Surface { surface: shown, .. } if shown == surface) {
            self.shown = Shown::default();
            return true;
        }
        false
    }

    pub(crate) fn image(&mut self, scale: f64) -> Arc<CursorImage> {
        let shape = match (self.decor, self.shown) {
            (Some(shape), _) | (None, Shown::Shape(shape)) => shape,
            (None, Shown::Hidden | Shown::Surface { .. }) => CursorShape::Default,
        };
        let scale = (scale.ceil() as u32).max(1);
        let size = self.size * scale;
        let theme = &self.theme;
        let fallback = &self.fallback;
        self.images
            .entry((shape, scale))
            .or_insert_with(|| image::load(theme, shape, size).unwrap_or_else(|| fallback.clone()))
            .clone()
    }

    pub(crate) fn position(&self) -> Point<f64, Logical> {
        self.position
    }

    pub(crate) fn move_to(&mut self, position: Point<f64, Logical>) -> Point<f64, Logical> {
        std::mem::replace(&mut self.position, position)
    }
}

impl fmt::Debug for Cursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Cursor")
            .field("position", &self.position)
            .field("shown", &self.shown)
            .field("theme", &self.named)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

fn machine_theme() -> String {
    std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "default".to_owned())
}
