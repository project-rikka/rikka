use rikka_psd::{
    ColorConverter, Compression, Document, Plane, ResourceData, Result, RgbaImage, resource_id,
};

use crate::raster::Raster;
use crate::render::Renderer;
use crate::space::{Space, SpaceKind};

impl Raster {
    pub(crate) fn to_rgba(&self, space: &Space) -> RgbaImage {
        let channels = self.channels;
        let mut straight = [0.0f32; 4];
        let pixels = self
            .data
            .chunks_exact(channels + 1)
            .map(|pixel| {
                let alpha = pixel[channels];
                if alpha <= 0.0 {
                    return [0.0; 4];
                }
                for channel in 0..channels {
                    straight[channel] = pixel[channel] / alpha;
                }
                let [r, g, b] = space.to_rgb(&straight);
                [r, g, b, alpha]
            })
            .collect();
        RgbaImage {
            width: self.width(),
            height: self.height(),
            pixels,
            linear: space.linear(),
        }
    }

    pub(crate) fn to_rgba_managed(&self, space: &Space, converter: &ColorConverter) -> RgbaImage {
        if space.kind != SpaceKind::Cmyk || !converter.is_managed() {
            return self.to_rgba(space);
        }
        let channels = self.channels;
        let count = self.width() as usize * self.height() as usize;
        let mut planes = vec![Vec::with_capacity(count); channels];
        let mut alphas = Vec::with_capacity(count);
        for pixel in self.data.chunks_exact(channels + 1) {
            let alpha = pixel[channels];
            for (channel, plane) in planes.iter_mut().enumerate() {
                plane.push(if alpha > 0.0 {
                    pixel[channel] / alpha
                } else {
                    1.0
                });
            }
            alphas.push(alpha);
        }
        let pixels = converter
            .to_rgb(&planes)
            .into_iter()
            .zip(alphas)
            .map(|([r, g, b], alpha)| {
                if alpha > 0.0 {
                    [r, g, b, alpha]
                } else {
                    [0.0; 4]
                }
            })
            .collect();
        RgbaImage {
            width: self.width(),
            height: self.height(),
            pixels,
            linear: space.linear(),
        }
    }
}

pub fn render_rgba(document: &Document) -> Result<RgbaImage> {
    let renderer = Renderer::new(document);
    Ok(renderer
        .render()?
        .to_rgba_managed(&renderer.space(), &document.color_converter()))
}
pub fn refresh_composite(document: &mut Document) -> Result<()> {
    if !matches!(
        document.color_mode,
        rikka_psd::ColorMode::Rgb
            | rikka_psd::ColorMode::Cmyk
            | rikka_psd::ColorMode::Grayscale
            | rikka_psd::ColorMode::Lab
            | rikka_psd::ColorMode::Duotone
    ) {
        return Err(rikka_psd::Error::ColorMode(document.color_mode.name()));
    }
    let renderer = Renderer::new(document);
    let space = renderer.space();
    let raster = renderer.render()?;
    let channels = space.channels();
    let count = raster.width() as usize * raster.height() as usize;
    let transparent = raster
        .data
        .chunks_exact(channels + 1)
        .any(|pixel| pixel[channels] < 1.0);
    let white = space.white();
    let mut planes = vec![Vec::with_capacity(count); channels + usize::from(transparent)];
    for pixel in raster.data.chunks_exact(channels + 1) {
        let alpha = pixel[channels].clamp(0.0, 1.0);
        for channel in 0..channels {
            let value = if transparent {
                pixel[channel] + white[channel] * (1.0 - alpha)
            } else if alpha > 0.0 {
                pixel[channel] / alpha
            } else {
                0.0
            };
            planes[channel].push(value.clamp(0.0, 1.0));
        }
        if transparent {
            planes[channels].push(alpha);
        }
    }
    let depth = document.depth;
    let (width, height) = (document.width, document.height);
    let planes = planes
        .iter()
        .map(|values| Plane::from_normalized(width, height, depth, values))
        .collect::<Result<Vec<_>>>()?;
    document.merged_alpha = transparent;
    document.set_composite(&planes, Compression::Rle)?;
    if let Some(ResourceData::VersionInfo(info)) =
        document.resources.get_mut(resource_id::VERSION_INFO)
    {
        info.has_real_merged_data = true;
    }
    Ok(())
}
