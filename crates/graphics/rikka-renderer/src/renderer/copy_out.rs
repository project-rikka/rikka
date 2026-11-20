use std::sync::Arc;

use ash::vk;
use rikka_types::Rect;

use crate::renderer::canvas::{Canvas, Onto, Painted, Plan};
use crate::renderer::draw::RenderOutput;
use crate::renderer::history::DamageSource;
use crate::renderer::readback::Readback;
use crate::target::OffscreenTarget;
use rikka_gpu::core::barrier::{MemoryAccess, record_memory_barrier};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::image::{ImageAccess, color_subresource_layers};
#[cfg(unix)]
use rikka_gpu::drm::SharedRows;
#[cfg(unix)]
use rikka_gpu::error::Error;
use rikka_gpu::error::Result;
use rikka_gpu::texture::TexelRect;

#[derive(Debug)]
pub enum CopySink<'a> {
    #[cfg(unix)]
    Rows(&'a SharedRows),
    Readback(&'a mut Readback),
}

#[cfg(unix)]
impl<'a> From<&'a SharedRows> for CopySink<'a> {
    fn from(rows: &'a SharedRows) -> Self {
        Self::Rows(rows)
    }
}

impl<'a> From<&'a mut Readback> for CopySink<'a> {
    fn from(readback: &'a mut Readback) -> Self {
        Self::Readback(readback)
    }
}

#[derive(Debug)]
#[must_use = "a target is drawn and copied out by handing this to draw()"]
pub struct CopiedDraw<'a> {
    target: &'a mut OffscreenTarget,
    sink: CopySink<'a>,
    missed: &'a [TexelRect],
}

impl OffscreenTarget {
    pub fn copied_into<'a>(
        &'a mut self,
        sink: impl Into<CopySink<'a>>,
        missed: &'a [TexelRect],
    ) -> CopiedDraw<'a> {
        CopiedDraw {
            target: self,
            sink: sink.into(),
            missed,
        }
    }
}

#[derive(Debug)]
pub struct CopyOut<'a> {
    into: CopyInto<'a>,
    missed: &'a [TexelRect],
}

#[derive(Debug)]
enum CopyInto<'a> {
    #[cfg(unix)]
    Buffer {
        buffer: vk::Buffer,
        row_texels: u32,
    },
    Readback(&'a mut Readback),
}

impl<'a> CopyOut<'a> {
    pub(crate) fn reserve(
        self,
        device: &Arc<RenderDevice>,
        completed: u64,
        extent: vk::Extent2D,
    ) -> Result<CopyTarget<'a>> {
        let (buffer, row_texels) = match self.into {
            #[cfg(unix)]
            CopyInto::Buffer { buffer, row_texels } => (buffer, row_texels),
            CopyInto::Readback(readback) => {
                (readback.reserve(device, completed, extent)?, extent.width)
            }
        };
        Ok(CopyTarget {
            buffer,
            row_texels,
            missed: self.missed,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CopyTarget<'a> {
    pub(crate) buffer: vk::Buffer,
    row_texels: u32,
    missed: &'a [TexelRect],
}

impl CopyTarget<'_> {
    pub(crate) fn regions(
        &self,
        drawn: impl IntoIterator<Item = Rect>,
        extent: vk::Extent2D,
        regions: &mut Vec<vk::BufferImageCopy>,
    ) {
        regions.clear();
        let areas = drawn
            .into_iter()
            .filter_map(texels_of)
            .chain(self.missed.iter().copied());
        for area in areas {
            if let Some(copy) = region(area, extent, self.row_texels)
                && !regions.iter().any(|kept| same_region(kept, &copy))
            {
                regions.push(copy);
            }
        }
    }
}

impl Onto for CopiedDraw<'_> {
    type Output = RenderOutput;

    fn canvas(&mut self) -> Canvas {
        let image = &*self.target.image;
        let plan = Plan::new(self.target.lease.id, image.extent())
            .transformed(self.target.transform)
            .damaged(DamageSource::CompleteForBuffer)
            .read_back(true)
            .viewed_by(self.target.viewer)
            .blended_in(self.target.space);
        Canvas::new(plan, image.handle(), image.view(), image.format())
            .with_layouts(image.access(), ImageAccess::TRANSFER_SRC)
    }

    fn copies_out(&mut self) -> Result<Option<CopyOut<'_>>> {
        let into = match &mut self.sink {
            #[cfg(unix)]
            CopySink::Rows(rows) => {
                let extent = self.target.image.extent();
                if (rows.width, rows.height) != (extent.width, extent.height) {
                    return Err(Error::Invalid(format!(
                        "{}x{} rows do not match a {}x{} target",
                        rows.width, rows.height, extent.width, extent.height
                    )));
                }
                CopyInto::Buffer {
                    buffer: rows.memory.buffer,
                    row_texels: rows.stride / 4,
                }
            }
            CopySink::Readback(readback) => CopyInto::Readback(readback),
        };
        Ok(Some(CopyOut {
            into,
            missed: self.missed,
        }))
    }

    fn has_copy_work(&self) -> bool {
        !self.missed.is_empty()
    }

    fn undrawn(&mut self, output: RenderOutput) -> RenderOutput {
        output
    }

    fn drawn(&mut self, painted: Painted<'_>) -> Result<RenderOutput> {
        self.target.image.set_access(ImageAccess::TRANSFER_SRC);
        match &mut self.sink {
            CopySink::Readback(readback) => {
                let extent = self.target.image.extent();
                readback.arm(painted.submitted, (extent.width, extent.height));
            }
            #[cfg(unix)]
            CopySink::Rows(_) => {}
        }
        Ok(painted.output)
    }
}

pub(crate) fn record_copy(
    device: &RenderDevice,
    cmd: vk::CommandBuffer,
    image: vk::Image,
    buffer: vk::Buffer,
    regions: &[vk::BufferImageCopy],
) {
    if regions.is_empty() {
        return;
    }
    unsafe {
        device.handle().cmd_copy_image_to_buffer(
            cmd,
            image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            buffer,
            regions,
        );
    }
    record_memory_barrier(
        device,
        cmd,
        MemoryAccess::new(
            vk::PipelineStageFlags2::COPY,
            vk::AccessFlags2::TRANSFER_WRITE,
        ),
        MemoryAccess::new(vk::PipelineStageFlags2::HOST, vk::AccessFlags2::HOST_READ),
    );
}

pub(crate) fn region(
    area: TexelRect,
    extent: vk::Extent2D,
    row_texels: u32,
) -> Option<vk::BufferImageCopy> {
    let [x, y] = area.offset;
    let width = area.extent[0].min(extent.width.saturating_sub(x));
    let height = area.extent[1].min(extent.height.saturating_sub(y));
    if width == 0 || height == 0 {
        return None;
    }
    Some(
        vk::BufferImageCopy::default()
            .buffer_offset((u64::from(y) * u64::from(row_texels) + u64::from(x)) * 4)
            .buffer_row_length(row_texels)
            .image_subresource(color_subresource_layers())
            .image_offset(vk::Offset3D {
                x: x as i32,
                y: y as i32,
                z: 0,
            })
            .image_extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            }),
    )
}

fn texels_of(rect: Rect) -> Option<TexelRect> {
    let x = rect.min_x().max(0.0).floor();
    let y = rect.min_y().max(0.0).floor();
    let width = (rect.max_x().ceil() - x).max(0.0) as u32;
    let height = (rect.max_y().ceil() - y).max(0.0) as u32;
    (width > 0 && height > 0).then(|| TexelRect::new([x as u32, y as u32], [width, height]))
}

fn same_region(a: &vk::BufferImageCopy, b: &vk::BufferImageCopy) -> bool {
    (a.buffer_offset, a.image_extent.width, a.image_extent.height)
        == (b.buffer_offset, b.image_extent.width, b.image_extent.height)
}
