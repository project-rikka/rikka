use rikka_image_resample::{Axis, Footprint, Job, Resampler, Threads};

use crate::color_type::ColorType;
use crate::error::{Error, Result};
use crate::filter::Filter;
use crate::pixels::{Pixels, Sample, by_depth};
use crate::raster::{Image, checked_byte_len};

const CHANNELS: usize = 4;

impl Image {
    pub fn resize_exact(&self, width: u32, height: u32, filter: Filter) -> Result<Image> {
        checked_byte_len(width, height, self.color())?;
        let (source_width, source_height) = self.dimensions();
        if source_width == 0 || source_height == 0 {
            return Err(Error::InvalidSize {
                width: source_width,
                height: source_height,
            });
        }
        let working = match self.color().bits_per_channel() {
            8 => ColorType::Rgba8,
            16 => ColorType::Rgba16,
            _ => ColorType::Rgba32F,
        };
        let source = self.in_color(working);
        let job = Job {
            x: Axis::fit(source_width as usize, width as usize),
            y: Axis::fit(source_height as usize, height as usize),
            footprint: Footprint::Filtered(filter.kernel()),
            threads: Threads::Available,
        };
        let translucent = self.color().has_alpha();
        let pixels = by_depth!(&source.pixels, samples => resampled(samples, job, translucent));
        let resized = Image::from_pixels(width, height, working, pixels)
            .with_icc_profile(self.icc_profile().map(Into::into));
        Ok(resized.convert(self.color()))
    }

    pub fn resize_to_fit(&self, max_width: u32, max_height: u32, filter: Filter) -> Result<Image> {
        let (width, height) = fit(self.dimensions(), (max_width, max_height));
        self.resize_exact(width, height, filter)
    }

    pub fn thumbnail(self, max_width: u32, max_height: u32) -> Result<Image> {
        let (width, height) = self.dimensions();
        if width <= max_width && height <= max_height {
            return Ok(self);
        }
        self.resize_to_fit(max_width, max_height, Filter::Lanczos3)
    }
}

fn resampled<S: Sample>(source: &[S], job: Job, translucent: bool) -> Pixels {
    let pitch = job.x.source_len * CHANNELS;
    let mut target = vec![S::default(); job.x.target_len * job.y.target_len * CHANNELS];
    let unit = 1.0 / S::FULL.scaled();
    Resampler::new().resample::<S, CHANNELS>(
        job,
        &mut target,
        |row, out| {
            let texels = source[row * pitch..(row + 1) * pitch]
                .as_chunks::<CHANNELS>()
                .0;
            for (premultiplied, texel) in out.as_chunks_mut::<CHANNELS>().0.iter_mut().zip(texels) {
                let alpha = texel[3].scaled();
                let coverage = if translucent { alpha * unit } else { 1.0 };
                *premultiplied = [
                    texel[0].scaled() * coverage,
                    texel[1].scaled() * coverage,
                    texel[2].scaled() * coverage,
                    alpha,
                ];
            }
        },
        |blended, out| {
            let premultiplied = blended.as_chunks::<CHANNELS>().0;
            for (texel, pixel) in out
                .as_chunks_mut::<CHANNELS>()
                .0
                .iter_mut()
                .zip(premultiplied)
            {
                let alpha = pixel[3];
                let straighten = match (translucent, alpha > 0.0) {
                    (false, _) => 1.0,
                    (true, true) => S::FULL.scaled() / alpha,
                    (true, false) => 0.0,
                };
                *texel = [
                    S::from_scaled(pixel[0] * straighten),
                    S::from_scaled(pixel[1] * straighten),
                    S::from_scaled(pixel[2] * straighten),
                    S::from_scaled(alpha),
                ];
            }
        },
    );
    S::wrap(target)
}

fn fit(source: (u32, u32), bounds: (u32, u32)) -> (u32, u32) {
    let horizontal = f64::from(bounds.0) / f64::from(source.0.max(1));
    let vertical = f64::from(bounds.1) / f64::from(source.1.max(1));
    let factor = horizontal.min(vertical);
    let scale = |side: u32| ((f64::from(side) * factor).round() as u32).max(1);
    (scale(source.0), scale(source.1))
}
