use rikka_gpu::{Compute, DmabufSpec};
use rikka_gpu::{Dispatch, ImageFormat, Kernel, VideoTarget};

use crate::error::{Error, Result};
use crate::fitted::Fitted;
use crate::imported::Imported;
use crate::preview::{self, Preview};

const BLOCKS_PER_GROUP: u32 = 8;
const PREVIEW_GROUP: u32 = 8;

pub struct Converter {
    compute: Compute,
    kernel: Kernel,
    shrink: Kernel,
}

impl Converter {
    pub fn new(compute: Compute) -> Result<Converter> {
        let kernel = compute.kernel(video_kernels::NV12_COMP)?;
        let shrink = compute.kernel(video_kernels::PREVIEW_COMP)?;
        Ok(Converter {
            compute,
            kernel,
            shrink,
        })
    }

    pub fn preview(&self, source: &Imported, most: (u32, u32)) -> Result<Preview> {
        let (width, height) = preview::fitted(source.extent(), most);
        let target = self.compute.image(width, height, ImageFormat::Rgba8)?;
        let (limit_width, limit_height) = source.extent();
        let (origin_x, origin_y) = source.origin();
        let push: Vec<u8> = [
            source.image.slot(),
            target.slot(),
            width,
            height,
            limit_width,
            limit_height,
            origin_x,
            origin_y,
        ]
        .into_iter()
        .flat_map(u32::to_ne_bytes)
        .collect();
        let groups = [
            width.div_ceil(PREVIEW_GROUP),
            height.div_ceil(PREVIEW_GROUP),
            1,
        ];
        let mut batch = self.compute.batch();
        batch.dispatch(
            &self.shrink,
            Dispatch::new(groups)
                .reads_foreign(&source.image)
                .writes(&target)
                .push(&push),
        );
        let download = batch.download(&target, [0, 0], [width, height]);
        source.read_by(batch.submit()?);
        Ok(Preview {
            width,
            height,
            rgba: download.wait()?,
        })
    }

    pub fn import(&self, spec: &DmabufSpec<'_>) -> Result<Imported> {
        Ok(Imported::new(self.compute.import_dmabuf(spec)?))
    }

    pub fn convert(
        &self,
        source: &Imported,
        target: VideoTarget,
        (width, height): (u32, u32),
    ) -> Result<()> {
        let [surface_width, surface_height] = target.extent;
        let target = self.compute.video_target(target)?;
        let (Some(luma), Some(chroma)) = (target.plane_slot(0), target.plane_slot(1)) else {
            return Err(Error::Planes);
        };
        let (source_width, source_height) = source.extent();
        let (origin_x, origin_y) = source.origin();
        let fitted = Fitted::compute((source_width, source_height), (width, height));
        let push: Vec<u8> = [
            source.image.slot(),
            luma,
            chroma,
            0,
            width,
            height,
            source_width,
            source_height,
            fitted.offset.0,
            fitted.offset.1,
            fitted.extent.0,
            fitted.extent.1,
            surface_width,
            surface_height,
            origin_x,
            origin_y,
        ]
        .into_iter()
        .flat_map(u32::to_ne_bytes)
        .collect();
        let groups = [
            surface_width.div_ceil(2).div_ceil(BLOCKS_PER_GROUP),
            surface_height.div_ceil(2).div_ceil(BLOCKS_PER_GROUP),
            1,
        ];
        let mut batch = self.compute.batch();
        batch.dispatch(
            &self.kernel,
            Dispatch::new(groups)
                .reads_foreign(&source.image)
                .writes_foreign(&target)
                .push(&push),
        );
        source.read_by(batch.submit()?);
        Ok(())
    }
}

impl std::fmt::Debug for Converter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Converter").finish_non_exhaustive()
    }
}
