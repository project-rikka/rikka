use std::path::Path;
use std::sync::Arc;

use rikka_image::{ColorType, Filter, Format, Frame, GifWriter, Image, read};
use rikka_types::Color;

use crate::error::ConvertError;
use crate::target::PictureFormat;
use crate::tuning::{Quality, Scale, Tuning};

const ICON_SIDE: u32 = 256;
const AVIF_SPEED: u8 = 6;

pub(crate) fn convert(
    source: &Path,
    format: PictureFormat,
    tuning: Tuning,
    draft: &Path,
) -> Result<(), ConvertError> {
    if is_drawing(source) {
        let drawing = || read(source).color_scheme(tuning.scheme);
        let drawn = match tuning.scale {
            Scale::LongestSide(side) => drawing().rasterize_svg_to_fit(side, side)?,
            scale => fitted(drawing().rasterize_svg()?, scale)?,
        };
        return encode(&drawn, format.image(), tuning.quality, draft);
    }
    let decoded = match format {
        PictureFormat::Gif => match animate(source, tuning.scale, draft)? {
            Some(still) => still,
            None => return Ok(()),
        },
        _ => read(source).decode()?,
    };
    let picture = fitted(decoded, tuning.scale)?;
    encode(&picture, format.image(), tuning.quality, draft)
}

pub(crate) fn shrink(source: &Path, scale: Scale, draft: &Path) -> Result<(), ConvertError> {
    let format = Format::from_path(source)
        .filter(|format| format.can_encode())
        .ok_or_else(|| {
            ConvertError::Other(format!("{} cannot be written back", source.display()))
        })?;
    let decoded = match format {
        Format::Gif => match animate(source, scale, draft)? {
            Some(still) => still,
            None => return Ok(()),
        },
        _ => read(source).decode()?,
    };
    let picture = fitted(decoded, scale)?;
    encode(&picture, format, Quality::High, draft)
}

pub(crate) fn is_drawing(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

fn animate(source: &Path, scale: Scale, draft: &Path) -> Result<Option<Image>, ConvertError> {
    let animation = read(source).animation()?;
    if !animation.is_animated() {
        return Ok(animation
            .into_frames()
            .into_iter()
            .next()
            .map(Frame::into_image));
    }
    let (width, height) = animation.dimensions();
    let (width, height) = scale.fit(width, height);
    let mut writer = GifWriter::create(draft, width, height, animation.loops())?;
    for frame in animation.into_frames() {
        let delay = frame.delay();
        let picture = fitted(frame.into_image(), scale)?;
        writer.push(Frame::new(picture, delay))?;
    }
    writer.finish()?;
    Ok(None)
}

fn fitted(picture: Image, scale: Scale) -> Result<Image, ConvertError> {
    let (width, height) = picture.dimensions();
    let (fit_width, fit_height) = scale.fit(width, height);
    if (fit_width, fit_height) == (width, height) {
        return Ok(picture);
    }
    Ok(picture.resize_exact(fit_width, fit_height, Filter::Lanczos3)?)
}

fn encode(
    picture: &Image,
    format: Format,
    quality: Quality,
    draft: &Path,
) -> Result<(), ConvertError> {
    let quality = match format {
        Format::Jpeg => quality.pick(85, 92, 97),
        Format::WebP => quality.pick(80, 90, 97),
        Format::Avif => quality.pick(60, 75, 90),
        _ => 100,
    };
    let prepared = match format {
        Format::Jpeg if picture.color().has_alpha() => Some(flatten_on_white(picture)?),
        Format::Ico => {
            let (width, height) = picture.dimensions();
            (width > ICON_SIDE || height > ICON_SIDE)
                .then(|| picture.resize_to_fit(ICON_SIDE, ICON_SIDE, Filter::Lanczos3))
                .transpose()?
        }
        _ => None,
    };
    prepared
        .as_ref()
        .unwrap_or(picture)
        .write(format)
        .quality(quality)
        .speed(AVIF_SPEED)
        .to(draft)?;
    Ok(())
}

fn flatten_on_white(picture: &Image) -> Result<Image, ConvertError> {
    let (width, height) = picture.dimensions();
    let white = Image::filled(width, height, ColorType::Rgba8, Color::WHITE)?;
    Ok(white
        .overlay(picture, 0, 0)
        .with_icc_profile(picture.icc_profile().map(Arc::from)))
}
