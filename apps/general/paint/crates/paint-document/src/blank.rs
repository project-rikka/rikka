use rikka_psd::{
    ChannelId, ColorMode, Compression, Node, Resolution, ResourceData, RgbaImage, resource_id,
};
use rikka_types::Color;

use crate::develop::DECODED_BYTES_PER_PIXEL;
use crate::error::{Error, Result};
use crate::{arrange, save};

const BACKGROUND: &str = "Background";
const LAYER: &str = "Layer 1";
const SCREEN_RESOLUTION: f64 = 72.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backdrop {
    Color(Color),
    Transparent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Blank {
    width: u32,
    height: u32,
    resolution: f64,
    backdrop: Backdrop,
    layer_name: String,
}

impl Blank {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Blank {
        Blank {
            width,
            height,
            resolution: SCREEN_RESOLUTION,
            backdrop: Backdrop::Color(Color::WHITE),
            layer_name: String::from(LAYER),
        }
    }

    #[must_use]
    pub fn resolution(mut self, pixels_per_inch: f64) -> Blank {
        self.resolution = pixels_per_inch;
        self
    }

    #[must_use]
    pub fn backdrop(mut self, backdrop: Backdrop) -> Blank {
        self.backdrop = backdrop;
        self
    }

    #[must_use]
    pub fn layer_name(mut self, name: impl Into<String>) -> Blank {
        self.layer_name = name.into();
        self
    }

    pub(crate) fn photoshop(&self) -> Result<rikka_psd::Document> {
        let (width, height) = (self.width, self.height);
        if width == 0 || height == 0 {
            return Err(Error::EmptyImage);
        }
        if !self.resolution.is_finite() || self.resolution <= 0.0 {
            return Err(Error::BadResolution);
        }
        rikka_psd::Version::Psb
            .check_dimensions(width, height)
            .map_err(|_| Error::TooLarge { width, height })?;
        crate::document::check_memory_budget(
            width,
            height,
            u64::from(width) * u64::from(height) * DECODED_BYTES_PER_PIXEL * 2,
        )?;
        let mut document =
            rikka_psd::Document::new(width, height, rikka_psd::Depth::Eight, ColorMode::Rgb);
        document.resources.set(
            resource_id::RESOLUTION,
            ResourceData::Resolution(Resolution::pixels_per_inch(self.resolution)),
        );
        let fill = match self.backdrop {
            Backdrop::Color(color) => [color.r, color.g, color.b, 1.0],
            Backdrop::Transparent => [0.0; 4],
        };
        let pixels = RgbaImage {
            width,
            height,
            pixels: vec![fill; width as usize * height as usize],
            linear: false,
        };
        let record = match self.backdrop {
            Backdrop::Color(_) => {
                let mut record = rikka_psd::Layer::new(BACKGROUND);
                document.set_layer_rgba(&mut record, 0, 0, &pixels, Compression::Rle)?;
                record.remove_channel(ChannelId::Transparency);
                record
            }
            Backdrop::Transparent => arrange::empty(&document, &self.layer_name)?,
        };
        document.layers.push(Node::Layer(record));
        document.set_composite_rgba(&pixels, Compression::Rle)?;
        save::merged(&mut document, true);
        save::refresh(&mut document, false)?;
        Ok(document)
    }
}
