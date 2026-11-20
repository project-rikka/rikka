use rikka_image_edit::{Depth, Mode};
use rikka_psd::ColorMode;

use super::Document;
use crate::state::Content;

impl Document {
    #[must_use]
    pub fn bits(&self) -> u8 {
        match &self.state.content {
            Content::Picture { picture, .. } => match picture.depth() {
                Depth::Eight => 8,
                Depth::Sixteen => 16,
                Depth::Float => 32,
            },
            Content::Photoshop { document, .. } => match document.depth {
                rikka_psd::Depth::One => 1,
                rikka_psd::Depth::Eight => 8,
                rikka_psd::Depth::Sixteen => 16,
                rikka_psd::Depth::ThirtyTwo => 32,
            },
        }
    }

    #[must_use]
    pub fn color_mode(&self) -> &'static str {
        match &self.state.content {
            Content::Picture { picture, .. } => match picture.mode() {
                Mode::Rgb => "RGB",
                Mode::Gray => "Grayscale",
            },
            Content::Photoshop { document, .. } => match document.color_mode {
                ColorMode::Bitmap => "Bitmap",
                ColorMode::Grayscale => "Grayscale",
                ColorMode::Indexed => "Indexed",
                ColorMode::Rgb => "RGB",
                ColorMode::Cmyk => "CMYK",
                ColorMode::Multichannel => "Multichannel",
                ColorMode::Duotone => "Duotone",
                ColorMode::Lab => "Lab",
            },
        }
    }
}
