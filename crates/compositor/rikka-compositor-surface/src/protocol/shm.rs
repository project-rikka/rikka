use rikka_compositor_geometry::Size;
use rikka_compositor_protocols::wl_buffer::{self, WlBuffer};
use rikka_compositor_protocols::wl_shm::{self, WlShm};
use rikka_compositor_protocols::wl_shm_pool::{self, WlShmPool};
use rikka_compositor_types::Fourcc;
use rikka_wayland_wire::Resource;

use crate::buffer::{ShmLayout, Storage};
use crate::host::SurfaceHost;
use crate::ids::{BufferId, PoolId};
use crate::shm::{GrowError, bytes_per_pixel, from_wire, to_wire};

pub(crate) fn bound<S: SurfaceHost>(state: &mut S, shm: WlShm) {
    for format in state.surfaces().shm_formats() {
        if let Ok(format) = wl_shm::Format::try_from(to_wire(*format)) {
            shm.format(format);
        }
    }
}

pub(crate) fn request<S: SurfaceHost>(state: &mut S, shm: WlShm, request: wl_shm::Request) {
    let wl_shm::Request::CreatePool { id, fd, size } = request else {
        return;
    };
    let Some(size) = usize::try_from(size).ok().filter(|size| *size > 0) else {
        shm.object().post_error(
            wl_shm::Error::InvalidStride.code(),
            &format!("invalid pool size {size}"),
        );
        return;
    };
    match state.surfaces().buffers.pools.create(fd, size) {
        Ok(pool) => id.set_key(pool),
        Err(error) => shm.object().post_error(
            wl_shm::Error::InvalidFd.code(),
            &format!("cannot map the pool: {error}"),
        ),
    }
}

pub(crate) fn pool<S: SurfaceHost>(state: &mut S, pool: WlShmPool, request: wl_shm_pool::Request) {
    let Some(pool_id) = pool.key::<PoolId>() else {
        return;
    };
    match request {
        wl_shm_pool::Request::Destroy => {}
        wl_shm_pool::Request::CreateBuffer {
            id,
            offset,
            width,
            height,
            stride,
            format,
        } => {
            let surfaces = state.surfaces();
            let format = format
                .ok()
                .map(|format| from_wire(u32::from(format)))
                .filter(|format| surfaces.shm_formats().contains(format));
            let Some(format) = format else {
                pool.post_error(wl_shm_pool::Error::InvalidFormat);
                return;
            };
            let Some(pool_len) = surfaces.buffers.pools.len(pool_id) else {
                return;
            };
            let Some(layout) = layout(pool_id, offset, width, height, stride, format, pool_len)
            else {
                pool.object().post_error(
                    wl_shm_pool::Error::InvalidStride.code(),
                    &format!(
                        "invalid buffer {width}x{height} at offset {offset} with stride {stride} in a pool of {pool_len} bytes"
                    ),
                );
                return;
            };
            let buffer =
                surfaces
                    .buffers
                    .create(id.clone(), Storage::Shm(layout), Size::new(width, height));
            id.set_key(buffer);
        }
        wl_shm_pool::Request::Resize { size } => {
            let Ok(size) = usize::try_from(size) else {
                pool.post_error(wl_shm_pool::Error::InvalidStride);
                return;
            };
            match state.surfaces().buffers.pools.grow(pool_id, size) {
                Ok(()) => {}
                Err(GrowError::Shrink) => pool.object().post_error(
                    wl_shm_pool::Error::InvalidStride.code(),
                    "shrinking a pool is not allowed",
                ),
                Err(GrowError::Map(error)) => pool.object().post_error(
                    wl_shm::Error::InvalidFd.code(),
                    &format!("cannot remap the pool: {error}"),
                ),
            }
        }
    }
}

pub(crate) fn pool_destroyed<S: SurfaceHost>(state: &mut S, pool: WlShmPool) {
    if let Some(pool) = pool.key::<PoolId>() {
        state.surfaces().buffers.pools.close(pool);
    }
}

pub(crate) fn buffer<S>(_state: &mut S, _buffer: WlBuffer, _request: wl_buffer::Request) {}

pub(crate) fn buffer_destroyed<S: SurfaceHost>(state: &mut S, buffer: WlBuffer) {
    if let Some(buffer) = buffer.key::<BufferId>() {
        state.surfaces().buffers.destroyed(buffer);
    }
}

fn layout(
    pool: PoolId,
    offset: i32,
    width: i32,
    height: i32,
    stride: i32,
    format: Fourcc,
    pool_len: usize,
) -> Option<ShmLayout> {
    let offset = usize::try_from(offset).ok()?;
    let width = usize::try_from(width).ok().filter(|width| *width > 0)?;
    let height = usize::try_from(height).ok().filter(|height| *height > 0)?;
    let stride = usize::try_from(stride).ok()?;
    let row = width.checked_mul(bytes_per_pixel(format).unwrap_or(1))?;
    let end = stride.checked_mul(height)?.checked_add(offset)?;
    (stride >= row && end <= pool_len).then_some(ShmLayout {
        pool,
        offset,
        stride,
        format,
    })
}
