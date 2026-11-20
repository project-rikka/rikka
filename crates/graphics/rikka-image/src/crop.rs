use crate::error::{Error, Result};
use crate::pixels::{Pixels, Sample, by_depth};
use crate::raster::Image;
use crate::region::Region;

impl Image {
    pub fn crop(&self, region: Region) -> Result<Image> {
        let (width, height) = self.dimensions();
        let region = region
            .clamp_to(width, height)
            .ok_or(Error::RegionOutOfBounds {
                region,
                width,
                height,
            })?;
        let channels = usize::from(self.color().channel_count());
        let pitch = width as usize * channels;
        let pixels = by_depth!(&self.pixels, samples => cut(samples, pitch, region, channels));
        Ok(self.derived(region.width, region.height, pixels))
    }
}

fn cut<S: Sample>(samples: &[S], pitch: usize, region: Region, channels: usize) -> Pixels {
    let row = region.width as usize * channels;
    let left = region.x as usize * channels;
    let mut out = Vec::with_capacity(row * region.height as usize);
    for line in samples
        .chunks_exact(pitch)
        .skip(region.y as usize)
        .take(region.height as usize)
    {
        out.extend_from_slice(&line[left..left + row]);
    }
    S::wrap(out)
}
