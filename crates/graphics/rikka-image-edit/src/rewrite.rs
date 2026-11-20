use rikka_gpu::ImageFormat;

use crate::error::Result;
use crate::ops::passes::CopyPush;
use crate::plane::{Plane, intersects};
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const COPY: u32 = 0;

pub(crate) fn rewritable(
    work: &mut Work,
    source: &Plane,
    format: ImageFormat,
    area: [u32; 4],
) -> Result<Plane> {
    let out = work.derived(source, format, |_, tile| intersects(tile, area))?;
    let tile_aligned = match format == source.format() {
        true => source.tile_aligned(area),
        false => [0, 0, source.width(), source.height()],
    };
    let copied = CopyPush {
        head: PushHeader::new(source, &out).mode(COPY),
        from: [0, 0],
        quarter: 0,
        mirror: 0,
        fill: [0.0; 4],
    };
    let io = Io::new(source, &out).limit_writes(tile_aligned);
    work.pass_at(Shader::Copy, copied, io, tile_aligned, 1.0)?;
    Ok(out)
}
