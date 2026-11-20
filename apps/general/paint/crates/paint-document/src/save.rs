use rikka_image::{ColorType, Compression as Packing, Format, Image};
use rikka_image_edit::{ConvertProfile, Depth, Mode, Picture, Profile, Recipe};
use rikka_psd::{ChannelId, Compression, Node, ResourceData, RgbaImage, VersionInfo, resource_id};
use rikka_psd_render::{refresh_composite, render_rgba};

use crate::develop::{self, DECODED_BYTES_PER_PIXEL, floats_of};
use crate::error::{Error, Result};

const THUMBNAIL: u32 = 160;
const WRITER: &str = "Rikka Paint";
pub(crate) const RENDERED_BYTES_PER_PIXEL: u64 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Saving {
    quality: u8,
    speed: u8,
    compression: Packing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Saved {
    Document,
    Copy,
}

impl Default for Saving {
    fn default() -> Saving {
        Saving {
            quality: 90,
            speed: 6,
            compression: Packing::Balanced,
        }
    }
}

impl Saving {
    #[must_use]
    pub fn new() -> Saving {
        Saving::default()
    }

    #[must_use]
    pub fn quality(mut self, quality: u8) -> Saving {
        self.quality = quality.clamp(1, 100);
        self
    }

    #[must_use]
    pub fn speed(mut self, speed: u8) -> Saving {
        self.speed = speed.clamp(1, 10);
        self
    }

    #[must_use]
    pub fn compression(mut self, compression: Packing) -> Saving {
        self.compression = compression;
        self
    }

    pub(crate) fn encode(&self, image: &Image, format: Format) -> Result<Vec<u8>> {
        let flattened = match format.supports_alpha() || !image.has_alpha() {
            true => None,
            false => Some(on_white(image)),
        };
        flattened
            .as_ref()
            .unwrap_or(image)
            .write(format)
            .quality(self.quality)
            .speed(self.speed)
            .compression(self.compression)
            .to_bytes()
            .map_err(Error::from)
    }
}

fn on_white(image: &Image) -> Image {
    let (width, height) = image.dimensions();
    let pixels: Vec<f32> = floats_of(image)
        .into_iter()
        .flat_map(|[r, g, b, a]| {
            let alpha = a.clamp(0.0, 1.0);
            [r, g, b]
                .map(|channel| channel * alpha + 1.0 - alpha)
                .into_iter()
                .chain([1.0])
        })
        .collect();
    let opaque = match image.color() {
        ColorType::La8 => ColorType::L8,
        ColorType::La16 => ColorType::L16,
        ColorType::Rgba16 => ColorType::Rgb16,
        ColorType::Rgba32F => ColorType::Rgb32F,
        _ => ColorType::Rgb8,
    };
    Image::from_rgba32f(width, height, pixels)
        .unwrap_or_else(|_| unreachable!("the pixels match their {width}×{height} size"))
        .convert(opaque)
        .with_icc_profile(image.icc_profile().map(Into::into))
}

pub(crate) fn picture_image(picture: &Picture) -> Result<Image> {
    picture.to_image().map_err(Error::from)
}

pub(crate) fn photoshop_image(
    document: &rikka_psd::Document,
    stale: bool,
    profile: Option<&Profile>,
    keeps_float: bool,
) -> Result<Image> {
    let rgba = match stale || !document.has_real_composite() {
        true => {
            check_render_budget(document)?;
            render_rgba(document)
        }
        false => document.composite_rgba(),
    }
    .map_err(Error::from)?;
    let linear = rgba.linear;
    let floats: Vec<f32> = rgba
        .pixels
        .iter()
        .flat_map(|&[r, g, b, a]| match linear && !keeps_float {
            true => [encode(r), encode(g), encode(b), a],
            false => [r, g, b, a],
        })
        .collect();
    let image = Image::from_rgba32f(rgba.width, rgba.height, floats).map_err(Error::from)?;
    let image = match document.depth {
        rikka_psd::Depth::ThirtyTwo if keeps_float => image,
        rikka_psd::Depth::ThirtyTwo | rikka_psd::Depth::Sixteen => image.convert(ColorType::Rgba16),
        _ => image.convert(ColorType::Rgba8),
    };
    let profile = match linear {
        true => None,
        false => profile.map(Profile::icc),
    };
    Ok(image.with_icc_profile(profile))
}

fn encode(value: f32) -> f32 {
    rikka_types::srgb::from_linear_f64(f64::from(value)) as f32
}

pub(crate) fn refresh(document: &mut rikka_psd::Document, stale: bool) -> Result<()> {
    if stale {
        check_render_budget(document)?;
        if refresh_composite(document).is_err() {
            merged(document, false);
        }
    }
    if stale || document.resources.thumbnail().is_none() {
        document.resources.remove(resource_id::THUMBNAIL_BGR);
        match thumbnail(document) {
            Some(thumbnail) => document
                .resources
                .set(resource_id::THUMBNAIL, ResourceData::Thumbnail(thumbnail)),
            None => {
                document.resources.remove(resource_id::THUMBNAIL);
            }
        }
    }
    Ok(())
}

pub(crate) fn sized_for(document: &mut rikka_psd::Document, large: bool) -> Result<()> {
    let version = match large {
        true => rikka_psd::Version::Psb,
        false => rikka_psd::Version::Psd,
    };
    version.check_dimensions(document.width, document.height)?;
    document.version = version;
    Ok(())
}

pub(crate) fn merged(document: &mut rikka_psd::Document, real: bool) {
    match document.resources.get_mut(resource_id::VERSION_INFO) {
        Some(ResourceData::VersionInfo(info)) => info.has_real_merged_data = real,
        _ => document.resources.set(
            resource_id::VERSION_INFO,
            ResourceData::VersionInfo(version_info(real)),
        ),
    }
}

fn version_info(real: bool) -> VersionInfo {
    VersionInfo {
        version: 1,
        has_real_merged_data: real,
        writer: String::from(WRITER),
        reader: String::from(WRITER),
        file_version: 1,
    }
}

fn thumbnail(document: &rikka_psd::Document) -> Option<rikka_psd::Thumbnail> {
    if !document.has_real_composite() {
        return None;
    }
    let pixels = document.composite_rgba8().ok()?;
    let shrunk = Image::from_rgba8(document.width, document.height, pixels)
        .ok()?
        .thumbnail(THUMBNAIL, THUMBNAIL)
        .ok()?;
    let rgb = on_white(&shrunk.convert(ColorType::Rgba8)).convert(ColorType::Rgb8);
    let (width, height) = rgb.dimensions();
    rikka_psd::Thumbnail::from_rgb8(width, height, rgb.as_bytes()).ok()
}

fn check_render_budget(document: &rikka_psd::Document) -> Result<()> {
    let (width, height) = (document.width, document.height);
    crate::document::check_memory_budget(
        width,
        height,
        u64::from(width) * u64::from(height) * RENDERED_BYTES_PER_PIXEL,
    )
}

pub(crate) fn picture_document(picture: &Picture, visible: bool) -> Result<rikka_psd::Document> {
    let (width, height) = picture.dimensions();
    crate::document::check_memory_budget(
        width,
        height,
        u64::from(width) * u64::from(height) * DECODED_BYTES_PER_PIXEL * 3,
    )?;
    let (depth, picture, linear) = match picture.depth() {
        Depth::Eight => (rikka_psd::Depth::Eight, picture.clone(), false),
        Depth::Sixteen => (rikka_psd::Depth::Sixteen, picture.clone(), false),
        Depth::Float => (
            rikka_psd::Depth::ThirtyTwo,
            picture.develop(&Recipe::new().then(ConvertProfile::new(Profile::linear_srgb())))?,
            true,
        ),
    };
    let mode = match picture.mode() {
        Mode::Gray => rikka_psd::ColorMode::Grayscale,
        Mode::Rgb => rikka_psd::ColorMode::Rgb,
    };
    let rgba = develop::rgba_of(&picture, linear)?;
    let opaque = rgba.pixels.iter().all(|pixel| pixel[3] >= 1.0);
    let mut document = rikka_psd::Document::new(width, height, depth, mode);
    let mut record = rikka_psd::Layer::new(match opaque {
        true => "Background",
        false => "Layer 0",
    });
    document.set_layer_rgba(&mut record, 0, 0, &rgba, Compression::Rle)?;
    if opaque {
        record.remove_channel(ChannelId::Transparency);
    }
    record.set_visible(visible);
    document.layers.push(Node::Layer(record));
    let composite = match visible {
        true => rgba,
        false => RgbaImage {
            pixels: vec![[0.0; 4]; width as usize * height as usize],
            ..rgba
        },
    };
    document.set_composite_rgba(&composite, Compression::Rle)?;
    let profile = match linear {
        true => Some(Profile::srgb()),
        false => picture.profile().cloned(),
    };
    if let Some(profile) = profile {
        document.resources.set(
            resource_id::ICC_PROFILE,
            ResourceData::IccProfile(profile.icc().to_vec()),
        );
    }
    merged(&mut document, true);
    refresh(&mut document, false)?;
    Ok(document)
}
