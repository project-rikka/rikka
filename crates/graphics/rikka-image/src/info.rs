use rikka_types::Size;

use crate::color_type::ColorType;
use crate::format::Format;
use crate::orientation::Orientation;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImageInfo {
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub color: ColorType,
    pub orientation: Orientation,
    pub text: Vec<(String, String)>,
}

impl ImageInfo {
    #[must_use]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[must_use]
    pub fn size(&self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }

    #[must_use]
    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0 {
            return 0.0;
        }
        self.width as f32 / self.height as f32
    }

    #[must_use]
    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    #[must_use]
    pub fn has_alpha(&self) -> bool {
        self.color.has_alpha()
    }
}
