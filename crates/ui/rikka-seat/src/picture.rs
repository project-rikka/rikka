use std::fs::File;
use std::io::Write as _;
use std::os::fd::{AsFd, OwnedFd};

use rikka_input::cursor::cursor_pixels;
use rikka_types::CursorImage;
use rikka_wayland_client::Route;
use rikka_wayland_client::wl_buffer::WlBuffer;
use rikka_wayland_client::wl_shm::{self, WlShm};
use rikka_wayland_client::wl_shm_pool::WlShmPool;
use rustix::fs::MemfdFlags;

const MEMFD_NAME: &str = "rikka-cursor";

pub(crate) struct Prepared {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) hotspot: (i32, i32),
    bgra: Vec<u8>,
}

impl Prepared {
    pub(crate) fn from_bgra(
        width: u32,
        height: u32,
        hotspot: (u32, u32),
        bgra: Vec<u8>,
    ) -> Option<Prepared> {
        let length = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        if length == 0 || bgra.len() != length {
            return None;
        }
        Some(Prepared {
            width: i32::try_from(width).ok()?,
            height: i32::try_from(height).ok()?,
            hotspot: (
                i32::try_from(hotspot.0).ok()?,
                i32::try_from(hotspot.1).ok()?,
            ),
            bgra,
        })
    }
}

pub(crate) fn prepared(image: CursorImage) -> Option<Prepared> {
    let pixels = cursor_pixels(image)?;
    Some(Prepared {
        width: i32::try_from(pixels.width).ok()?,
        height: i32::try_from(pixels.height).ok()?,
        hotspot: pixels.hotspot,
        bgra: pixels.premultiplied_bgra.clone(),
    })
}

pub(crate) struct Uploaded {
    pub(crate) buffer: WlBuffer,
    pool: WlShmPool,
    _memory: OwnedFd,
}

impl Uploaded {
    pub(crate) fn new(shm: &WlShm, picture: &Prepared) -> Option<Uploaded> {
        let memory = rustix::fs::memfd_create(MEMFD_NAME, MemfdFlags::CLOEXEC)
            .inspect_err(|error| tracing::debug!("cannot make cursor memory: {error}"))
            .ok()?;
        let mut file = File::from(memory);
        if let Err(error) = file.write_all(&picture.bgra) {
            tracing::debug!("cannot fill cursor memory: {error}");
            return None;
        }
        let memory = OwnedFd::from(file);
        let size = i32::try_from(picture.bgra.len()).ok()?;
        let pool = shm.via(&Route::discard()).create_pool(memory.as_fd(), size);
        let buffer = pool.create_buffer(
            0,
            picture.width,
            picture.height,
            picture.width * 4,
            wl_shm::Format::Argb8888,
        );
        Some(Uploaded {
            buffer,
            pool,
            _memory: memory,
        })
    }
}

impl Drop for Uploaded {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
    }
}
