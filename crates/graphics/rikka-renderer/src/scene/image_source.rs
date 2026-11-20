use rikka_types::{Rect, Size};

use rikka_types::Transform;

use crate::renderer::ColorId;
use crate::transform::sample_span;

const MINIFIED: f32 = 1.25;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[must_use]
pub struct ImageSource {
    pub(crate) region: Option<Rect>,
    pub(crate) transform: Transform,
    pub(crate) pixelated: bool,
    pub(crate) color: ColorId,
}

impl ImageSource {
    pub fn whole() -> Self {
        Self::default()
    }

    pub fn region(mut self, region: Rect) -> Self {
        self.region = Some(region);
        self
    }

    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    pub fn pixelated(mut self, pixelated: bool) -> Self {
        self.pixelated = pixelated;
        self
    }

    pub fn color(mut self, color: ColorId) -> Self {
        self.color = color;
        self
    }

    pub(crate) fn sample(&self, extent: [u32; 2]) -> ([f32; 2], [f32; 2], bool) {
        const WHOLE: Rect = Rect::new(0.0, 0.0, 1.0, 1.0);

        let uv = match self.region {
            Some(region) => {
                let (w, h) = (extent[0] as f32, extent[1] as f32);
                Rect::new(
                    region.min_x() / w,
                    region.min_y() / h,
                    region.width() / w,
                    region.height() / h,
                )
            }
            None => WHOLE,
        };
        sample_span(self.transform, uv)
    }

    pub(crate) fn minifies(&self, extent: [u32; 2], drawn: Size) -> bool {
        if self.pixelated {
            return false;
        }
        let (across, down) = match self.region {
            Some(region) => (region.width(), region.height()),
            None => (extent[0] as f32, extent[1] as f32),
        };
        let (across, down) = match self.transform.swaps_axes() {
            true => (down, across),
            false => (across, down),
        };
        across > drawn.width * MINIFIED || down > drawn.height * MINIFIED
    }
}
