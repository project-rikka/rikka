use rikka_image::{ColorType, Image};
use rikka_image_edit::{Imaging, Picture, Profile};
use rikka_psd::{ChannelId, Compression, RgbaImage};

use crate::error::{Error, Result};
use crate::layer::{self, LayerId};

pub(crate) const DECODED_BYTES_PER_PIXEL: u64 = 16;

#[derive(Clone, Debug)]
pub(crate) struct DevelopedLayer {
    pub(crate) record: rikka_psd::Layer,
    pub(crate) picture: Picture,
    linear: bool,
}

pub(crate) fn pixel_layer(
    document: &rikka_psd::Document,
    id: LayerId,
) -> Result<&rikka_psd::Layer> {
    let Some(rikka_psd::Node::Layer(record)) = layer::node(document, id) else {
        return Err(Error::NotPixelLayer(id));
    };
    match record.kind() {
        rikka_psd::LayerKind::Pixel => Ok(record),
        _ => Err(Error::NotPixelLayer(id)),
    }
}

pub(crate) fn placed(
    imaging: &Imaging,
    document: &rikka_psd::Document,
    profile: Option<&Profile>,
    id: LayerId,
) -> Result<DevelopedLayer> {
    let record = pixel_layer(document, id)?;
    let (width, height) = (document.width, document.height);
    crate::document::check_memory_budget(
        width,
        height,
        u64::from(width) * u64::from(height) * DECODED_BYTES_PER_PIXEL * 2,
    )?;
    let layer_rgba = document.layer_rgba(record)?;
    let mut canvas = vec![[0.0f32; 4]; width as usize * height as usize];
    let bounds = record.bounds;
    for row in 0..layer_rgba.height {
        let y = i64::from(bounds.top) + i64::from(row);
        if y < 0 || y >= i64::from(height) {
            continue;
        }
        for column in 0..layer_rgba.width {
            let x = i64::from(bounds.left) + i64::from(column);
            if x < 0 || x >= i64::from(width) {
                continue;
            }
            canvas[y as usize * width as usize + x as usize] =
                layer_rgba.pixels[(row * layer_rgba.width + column) as usize];
        }
    }
    let rgba = RgbaImage {
        width,
        height,
        pixels: canvas,
        linear: layer_rgba.linear,
    };
    let background = layer::is_opaque_record(record);
    Ok(DevelopedLayer {
        record: record.clone(),
        picture: pictured(imaging, &rgba, document.depth, profile, background)?,
        linear: layer_rgba.linear,
    })
}

pub(crate) fn picture_of(
    imaging: &Imaging,
    rgba: &RgbaImage,
    depth: rikka_psd::Depth,
    profile: Option<&Profile>,
) -> Result<Picture> {
    pictured(imaging, rgba, depth, profile, false)
}

fn pictured(
    imaging: &Imaging,
    rgba: &RgbaImage,
    depth: rikka_psd::Depth,
    profile: Option<&Profile>,
    opaque: bool,
) -> Result<Picture> {
    let floats: Vec<f32> = rgba.pixels.iter().flatten().copied().collect();
    let image = Image::from_rgba32f(rgba.width, rgba.height, floats)?;
    let image = match opaque {
        true => solid_at(image, depth),
        false => stored_at(image, depth),
    };
    let profile = match rgba.linear {
        true => Some(Profile::linear_srgb()),
        false => profile.cloned(),
    };
    Ok(imaging.picture(&image)?.with_profile(profile))
}

pub(crate) fn stored_at(image: Image, depth: rikka_psd::Depth) -> Image {
    match depth {
        rikka_psd::Depth::ThirtyTwo => image,
        rikka_psd::Depth::Sixteen => image.convert(ColorType::Rgba16),
        _ => image.convert(ColorType::Rgba8),
    }
}

fn solid_at(image: Image, depth: rikka_psd::Depth) -> Image {
    match depth {
        rikka_psd::Depth::ThirtyTwo => image.convert(ColorType::Rgb32F),
        rikka_psd::Depth::Sixteen => image.convert(ColorType::Rgb16),
        _ => image.convert(ColorType::Rgb8),
    }
}

pub(crate) fn floats_of(image: &Image) -> Vec<[f32; 4]> {
    image
        .convert(ColorType::Rgba32F)
        .as_bytes()
        .as_chunks::<16>()
        .0
        .iter()
        .map(|bytes| {
            let channel = |at: usize| {
                f32::from_ne_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
            };
            [channel(0), channel(4), channel(8), channel(12)]
        })
        .collect()
}

pub(crate) fn rgba_of(picture: &Picture, linear: bool) -> Result<RgbaImage> {
    Ok(RgbaImage {
        width: picture.width(),
        height: picture.height(),
        pixels: floats_of(&picture.to_image()?),
        linear,
    })
}

impl DevelopedLayer {
    pub(crate) fn new(record: rikka_psd::Layer, picture: Picture, linear: bool) -> DevelopedLayer {
        DevelopedLayer {
            record,
            picture,
            linear,
        }
    }

    pub(crate) fn linear(&self) -> bool {
        self.linear
    }

    pub(crate) fn rewritten_layer(
        &self,
        document: &rikka_psd::Document,
        developed: &Picture,
        compression: Compression,
    ) -> Result<rikka_psd::Layer> {
        rewritten(
            document,
            self.record.clone(),
            developed,
            self.linear,
            compression,
        )
    }
}

pub(crate) fn rewritten(
    document: &rikka_psd::Document,
    mut record: rikka_psd::Layer,
    picture: &Picture,
    linear: bool,
    compression: Compression,
) -> Result<rikka_psd::Layer> {
    let background = layer::is_opaque_record(&record);
    let rgba = rgba_of(picture, linear)?;
    document.set_layer_rgba(&mut record, 0, 0, &rgba, compression)?;
    if background {
        record.remove_channel(ChannelId::Transparency);
    }
    Ok(record)
}
