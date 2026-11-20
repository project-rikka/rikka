use std::fs::File;
use std::ops::Range;
use std::os::fd::AsFd as _;
use std::os::unix::fs::FileExt as _;

use rikka_wayland_client::wl_buffer::WlBuffer;
use rikka_wayland_client::wl_shm::{self, WlShm};
use rikka_wayland_client::wl_shm_pool::WlShmPool;
use rustix::fs::{MemfdFlags, memfd_create};

const BYTES_PER_PIXEL: i32 = 4;

#[derive(Debug)]
pub struct Canvas {
    pub buffer: WlBuffer,
    pub pool: WlShmPool,
    pub memory: File,
    pub width: i32,
    pub height: i32,
}

impl Canvas {
    pub fn new(shm: &WlShm, width: i32, height: i32, format: wl_shm::Format) -> Self {
        assert!(
            holds(format),
            "a canvas holds 32-bit pixels, not {format:?}"
        );
        let size = width * height * BYTES_PER_PIXEL;
        let memory = File::from(
            memfd_create("rikka-test-canvas", MemfdFlags::CLOEXEC).expect("a memfd for pixels"),
        );
        memory.set_len(size as u64).expect("room for the pixels");
        let pool = shm.create_pool(memory.as_fd(), size);
        let buffer = pool.create_buffer(0, width, height, width * BYTES_PER_PIXEL, format);
        Self {
            buffer,
            pool,
            memory,
            width,
            height,
        }
    }

    pub fn read(&self, x: i32, y: i32) -> [u8; 4] {
        let mut pixel = [0; 4];
        self.memory
            .read_exact_at(&mut pixel, self.offset(x, y))
            .expect("the pixel is inside the canvas");
        pixel
    }

    pub fn fill(&self, rows: Range<i32>, bgra: [u8; 4]) {
        self.paint(rows, 0..self.width, |_, _| bgra);
    }

    pub fn paint_each(&self, bgra: impl Fn(i32, i32) -> [u8; 4]) {
        self.paint(0..self.height, 0..self.width, bgra);
    }

    pub fn paint(&self, rows: Range<i32>, columns: Range<i32>, bgra: impl Fn(i32, i32) -> [u8; 4]) {
        let columns = columns.start.max(0)..columns.end.min(self.width);
        let mut row = Vec::with_capacity(columns.len() * BYTES_PER_PIXEL as usize);
        for y in rows.start.max(0)..rows.end.min(self.height) {
            row.clear();
            row.extend(columns.clone().flat_map(|x| bgra(x, y)));
            self.memory
                .write_all_at(&row, self.offset(columns.start, y))
                .expect("the row is inside the canvas");
        }
    }

    fn offset(&self, x: i32, y: i32) -> u64 {
        ((y * self.width + x) * BYTES_PER_PIXEL) as u64
    }
}

fn holds(format: wl_shm::Format) -> bool {
    matches!(
        format,
        wl_shm::Format::Argb8888
            | wl_shm::Format::Xrgb8888
            | wl_shm::Format::Abgr8888
            | wl_shm::Format::Xbgr8888
            | wl_shm::Format::Rgba8888
            | wl_shm::Format::Rgbx8888
            | wl_shm::Format::Bgra8888
            | wl_shm::Format::Bgrx8888
            | wl_shm::Format::Argb2101010
            | wl_shm::Format::Xrgb2101010
            | wl_shm::Format::Abgr2101010
            | wl_shm::Format::Xbgr2101010
    )
}
