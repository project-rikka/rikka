use std::path::Path;

use rikka_gpu::ImageFormat;
use rikka_image::{ColorType, Image};

use crate::error::Result;
use crate::picture::{Depth, Mode, Picture};
use crate::plane::Plane;
use crate::work::Work;

impl Picture {
    pub fn to_image(&self) -> Result<Image> {
        let frame = self.frame();
        let bytes = download(&mut Work::new(self.imaging()), &frame.plane)?;
        let (width, height) = self.dimensions();
        let image = match frame.plane.format() {
            ImageFormat::Rgba8 => Image::from_rgba8(width, height, bytes)?,
            ImageFormat::Rgba16 => {
                Image::from_rgba16(width, height, bytemuck::pod_collect_to_vec(&bytes))?
            }
            _ => Image::from_rgba32f(width, height, bytemuck::pod_collect_to_vec(&bytes))?,
        };
        let target = match (frame.depth, frame.mode, frame.has_alpha) {
            (Depth::Eight, Mode::Gray, false) => ColorType::L8,
            (Depth::Eight, Mode::Gray, true) => ColorType::La8,
            (Depth::Eight, Mode::Rgb, false) => ColorType::Rgb8,
            (Depth::Eight, Mode::Rgb, true) => ColorType::Rgba8,
            (Depth::Sixteen, Mode::Gray, false) => ColorType::L16,
            (Depth::Sixteen, Mode::Gray, true) => ColorType::La16,
            (Depth::Sixteen, Mode::Rgb, false) => ColorType::Rgb16,
            (Depth::Sixteen, Mode::Rgb, true) => ColorType::Rgba16,
            (Depth::Float, _, false) => ColorType::Rgb32F,
            (Depth::Float, _, true) => ColorType::Rgba32F,
        };
        Ok(image
            .convert(target)
            .with_icc_profile(self.profile().map(|profile| profile.icc())))
    }

    pub fn to_rgba8(&self) -> Result<Vec<u8>> {
        let frame = self.frame();
        if frame.plane.format() != ImageFormat::Rgba8 {
            return Ok(self.to_image()?.into_rgba8());
        }
        let mut bytes = download(&mut Work::new(self.imaging()), &frame.plane)?;
        if !frame.has_alpha {
            for pixel in bytes.as_chunks_mut::<4>().0 {
                pixel[3] = u8::MAX;
            }
        }
        Ok(bytes)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(self.to_image()?.save(path)?)
    }
}

pub(crate) fn download(work: &mut Work, plane: &Plane) -> Result<Vec<u8>> {
    let pixel = plane.format().bytes_per_pixel() as usize;
    let stride = plane.width() as usize * pixel;
    let mut pending: Vec<_> = plane
        .tiles_with_areas()
        .map(|([left, top, width, height], tile)| {
            (
                [left, top],
                [width, height],
                work.batch().download(tile, [0, 0], [width, height]),
            )
        })
        .collect();
    work.submit()?;
    if pending.len() == 1
        && let Some((_, _, whole)) = pending.pop()
    {
        return Ok(whole.wait()?);
    }
    let mut bytes = vec![0u8; stride * plane.height() as usize];
    for (at, [width, _], download) in pending {
        let tile = download.wait()?;
        let row = width as usize * pixel;
        for (line, source) in tile.chunks_exact(row).enumerate() {
            let start = (at[1] as usize + line) * stride + at[0] as usize * pixel;
            bytes[start..start + row].copy_from_slice(source);
        }
    }
    Ok(bytes)
}
