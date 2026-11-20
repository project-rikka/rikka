use rikka_svg::{Bitmap, ColorScheme, Svg};
use rikka_types::Source;

use crate::error::Result;
use crate::raster::Image;
use crate::read::read;

pub(crate) struct Rasterized {
    pub(crate) image: Image,
    pub(crate) follows_scheme: bool,
}

pub(crate) fn rasterize(
    source: &Source<'_>,
    width: u32,
    height: u32,
    scheme: ColorScheme,
) -> Result<Rasterized> {
    let document = open(source, scheme)?;
    Ok(Rasterized {
        image: render(&document, width, height)?,
        follows_scheme: document.follows_color_scheme(),
    })
}

pub(crate) fn rasterize_to_fit(
    source: &Source<'_>,
    bounds: Option<(u32, u32)>,
    scheme: ColorScheme,
) -> Result<Image> {
    let document = open(source, scheme)?;
    let natural = document.size();
    let (width, height) = (natural.width.max(1.0), natural.height.max(1.0));
    let scale = bounds.map_or(1.0, |(most_width, most_height)| {
        (most_width as f32 / width).min(most_height as f32 / height)
    });
    let pixels = |side: f32| (side * scale).round().max(1.0) as u32;
    render(&document, pixels(width), pixels(height))
}

fn open(source: &Source<'_>, scheme: ColorScheme) -> Result<Svg> {
    let embedded = |bytes: &[u8]| {
        let image = read(bytes).decode().ok()?;
        Some(Bitmap {
            width: image.width(),
            height: image.height(),
            rgba: image.into_rgba8(),
        })
    };
    let loader = Svg::decoding(&embedded).color_scheme(scheme);
    Ok(match source {
        Source::File(path) => loader.open(path)?,
        Source::Bytes(bytes) => loader.from_bytes(bytes)?,
    })
}

fn render(document: &Svg, width: u32, height: u32) -> Result<Image> {
    let raster = document.render(width, height)?;
    Image::from_rgba8(raster.width(), raster.height(), raster.into_pixels())
}
