use super::Document;
use super::header::ColorMode;
use crate::compression::{Compression, Encoded};
use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::layers::{ChannelId, Layer};
use crate::pixels::{ColorConverter, Plane, RgbaImage, encode_rgba8};
use crate::resources::{ResourceData, id as resource_id};

const BAND_PIXELS: usize = 1 << 16;

fn unmatted([r, g, b]: [f32; 3], alpha: Option<f32>) -> [f32; 4] {
    let Some(alpha) = alpha else {
        return [r, g, b, 1.0];
    };
    if alpha <= 0.0 {
        return [0.0; 4];
    }
    let unmatte = |value: f32| ((value - (1.0 - alpha)) / alpha).clamp(0.0, 1.0);
    [unmatte(r), unmatte(g), unmatte(b), alpha]
}

impl Document {
    pub fn color_converter(&self) -> ColorConverter {
        ColorConverter::new(self)
    }

    pub fn composite_planes(&self) -> Result<Vec<Plane>> {
        self.composite
            .planes
            .iter()
            .map(|plane| plane.decode(self.width, self.height, self.depth))
            .collect()
    }

    pub fn composite_plane(&self, index: usize) -> Result<Option<Plane>> {
        self.composite
            .planes
            .get(index)
            .map(|plane| plane.decode(self.width, self.height, self.depth))
            .transpose()
    }

    pub fn set_composite(&mut self, planes: &[Plane], compression: Compression) -> Result<()> {
        if planes.iter().any(|plane| {
            plane.width() != self.width
                || plane.height() != self.height
                || plane.depth() != self.depth
        }) {
            return Err(Error::Planes(
                "composite planes must match the document size and depth",
            ));
        }
        if planes.len() < self.color_mode.color_channels() {
            return Err(Error::Planes("the composite lacks colour channels"));
        }
        let compression = match compression {
            Compression::Raw => Compression::Raw,
            _ => Compression::Rle,
        };
        self.composite.planes = planes
            .iter()
            .map(|plane| Encoded::encode(plane, compression))
            .collect();
        Ok(())
    }

    pub fn composite_alpha_index(&self) -> Option<usize> {
        let color = self.color_mode.color_channels();
        (self.merged_alpha && self.composite.planes.len() > color).then_some(color)
    }

    pub fn has_real_composite(&self) -> bool {
        if self.composite.planes.len() < self.color_mode.color_channels() {
            return false;
        }
        let merged = match self.resources.get(resource_id::VERSION_INFO) {
            Some(ResourceData::VersionInfo(info)) => info.has_real_merged_data,
            _ => true,
        };
        merged || self.layers.is_empty()
    }

    pub fn composite_rgba(&self) -> Result<RgbaImage> {
        let converter = self.color_converter();
        let mut pixels = Vec::new();
        self.composite_bands(&converter, |band| pixels.extend_from_slice(band))?;
        Ok(RgbaImage {
            width: self.width,
            height: self.height,
            pixels,
            linear: converter.is_linear(),
        })
    }

    pub fn composite_rgba8(&self) -> Result<Vec<u8>> {
        let converter = self.color_converter();
        let linear = converter.is_linear();
        let mut bytes = Vec::new();
        self.composite_bands(&converter, |band| {
            let start = bytes.len();
            bytes.resize(start + band.len() * 4, 0);
            encode_rgba8(band, linear, &mut bytes[start..]);
        })?;
        Ok(bytes)
    }

    fn composite_bands(
        &self,
        converter: &ColorConverter,
        mut take: impl FnMut(&[[f32; 4]]),
    ) -> Result<()> {
        let color = self
            .composite
            .planes
            .iter()
            .take(converter.channels())
            .map(|plane| plane.decode(self.width, self.height, self.depth))
            .collect::<Result<Vec<_>>>()?;
        if color.len() < converter.channels() {
            return Err(Error::Planes("the composite lacks colour channels"));
        }
        let alpha = match self.composite_alpha_index() {
            Some(index) => self.composite_plane(index)?,
            None => None,
        };
        let rows = (BAND_PIXELS / self.width as usize).max(1);
        let mut pixels = Vec::with_capacity(rows * self.width as usize);
        for top in (0..self.height).step_by(rows) {
            let band = top..self.height.min(top.saturating_add(rows as u32));
            let channels: Vec<Vec<f32>> = color
                .iter()
                .map(|plane| plane.normalized_rows(band.clone()))
                .collect();
            let alpha = alpha
                .as_ref()
                .map(|plane| plane.normalized_rows(band.clone()));
            pixels.clear();
            pixels.extend(
                converter
                    .to_rgb(&channels)
                    .into_iter()
                    .enumerate()
                    .map(|(index, rgb)| unmatted(rgb, alpha.as_ref().map(|alpha| alpha[index]))),
            );
            take(&pixels);
        }
        Ok(())
    }

    pub fn set_composite_rgba(
        &mut self,
        image: &RgbaImage,
        compression: Compression,
    ) -> Result<()> {
        if image.width != self.width || image.height != self.height {
            return Err(Error::Planes("the image does not match the document size"));
        }
        let transparent = image.pixels.iter().any(|pixel| pixel[3] < 1.0);
        let rgb: Vec<[f32; 3]> = image
            .pixels
            .iter()
            .map(|pixel| {
                let a = pixel[3].clamp(0.0, 1.0);
                if transparent {
                    [0, 1, 2].map(|channel| pixel[channel] * a + (1.0 - a))
                } else {
                    [pixel[0], pixel[1], pixel[2]]
                }
            })
            .collect();
        let converter = self.color_converter();
        let mut planes: Vec<Plane> = converter
            .from_rgb(&rgb)
            .iter()
            .map(|values| Plane::from_normalized(self.width, self.height, self.depth, values))
            .collect::<Result<_>>()?;
        if transparent {
            let alpha: Vec<f32> = image.pixels.iter().map(|pixel| pixel[3]).collect();
            planes.push(Plane::from_normalized(
                self.width,
                self.height,
                self.depth,
                &alpha,
            )?);
        }
        self.merged_alpha = transparent;
        self.set_composite(&planes, compression)
    }

    pub fn layer_rgba(&self, layer: &Layer) -> Result<RgbaImage> {
        let bounds = layer.bounds;
        let count = bounds.area() as usize;
        let converter = self.color_converter();
        let mut color = Vec::with_capacity(converter.channels());
        for index in 0..converter.channels() {
            let plane = layer.decode_channel(ChannelId::Color(index as u16), self.depth)?;
            color.push(plane.map_or_else(
                || {
                    vec![
                        if self.color_mode == ColorMode::Cmyk {
                            1.0
                        } else {
                            0.0
                        };
                        count
                    ]
                },
                |plane| plane.to_normalized(),
            ));
        }
        let rgb = converter.to_rgb(&color);
        let alpha = layer
            .decode_channel(ChannelId::Transparency, self.depth)?
            .map(|plane| plane.to_normalized());
        let pixels = rgb
            .into_iter()
            .enumerate()
            .map(|(index, [r, g, b])| [r, g, b, alpha.as_ref().map_or(1.0, |alpha| alpha[index])])
            .collect();
        Ok(RgbaImage {
            width: bounds.width(),
            height: bounds.height(),
            pixels,
            linear: converter.is_linear(),
        })
    }

    pub fn set_layer_rgba(
        &self,
        layer: &mut Layer,
        left: i32,
        top: i32,
        image: &RgbaImage,
        compression: Compression,
    ) -> Result<()> {
        let bounds = Rect::from_size(left, top, image.width, image.height);
        let converter = self.color_converter();
        let rgb: Vec<[f32; 3]> = image
            .pixels
            .iter()
            .map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        let mut planes = Vec::new();
        let alpha: Vec<f32> = image.pixels.iter().map(|pixel| pixel[3]).collect();
        planes.push((
            ChannelId::Transparency,
            Plane::from_normalized(image.width, image.height, self.depth, &alpha)?,
        ));
        for (index, values) in converter.from_rgb(&rgb).into_iter().enumerate() {
            planes.push((
                ChannelId::Color(index as u16),
                Plane::from_normalized(image.width, image.height, self.depth, &values)?,
            ));
        }
        layer.set_pixels(bounds, &planes, compression)
    }

    pub fn layer_mask_plane(&self, layer: &Layer) -> Result<Option<Plane>> {
        layer.decode_channel(ChannelId::UserMask, self.depth)
    }
}
