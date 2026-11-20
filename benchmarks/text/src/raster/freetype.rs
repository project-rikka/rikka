use std::ffi::{c_int, c_long};
use std::ptr::NonNull;

use rikka_text_font::GlyphId;

use crate::masks::{Mask, subpixel_offset};

enum Bench {}

#[repr(C)]
struct Bitmap {
    width: u32,
    rows: u32,
    pitch: i32,
    left: i32,
    top: i32,
    buffer: *const u8,
}

unsafe extern "C" {
    fn ft_bench_open(data: *const u8, length: usize, index: c_long) -> *mut Bench;
    fn ft_bench_close(bench: *mut Bench);
    fn ft_bench_set_axis(bench: *mut Bench, tag: u32, value: f32) -> c_int;
    fn ft_bench_set_size(bench: *mut Bench, ppem: f32) -> c_int;
    fn ft_bench_render(bench: *mut Bench, glyph: u32, offset_x: f32, out: *mut Bitmap) -> c_int;
    fn ft_bench_outline(bench: *mut Bench, glyph: u32) -> c_long;
}

pub struct FreeType {
    bench: NonNull<Bench>,
}

impl FreeType {
    pub fn open(data: &'static [u8], index: u32) -> Option<Self> {
        let bench = unsafe { ft_bench_open(data.as_ptr(), data.len(), c_long::from(index)) };
        NonNull::new(bench).map(|bench| Self { bench })
    }

    #[must_use]
    pub fn weight(self, weight: Option<f32>) -> Self {
        if let Some(weight) = weight {
            unsafe { ft_bench_set_axis(self.bench.as_ptr(), u32::from_be_bytes(*b"wght"), weight) };
        }
        self
    }

    fn sized(&mut self, ppem: f32) {
        unsafe { ft_bench_set_size(self.bench.as_ptr(), ppem) };
    }

    fn bitmap(&mut self, glyph: GlyphId, offset: f32) -> Option<Bitmap> {
        let mut bitmap = Bitmap {
            width: 0,
            rows: 0,
            pitch: 0,
            left: 0,
            top: 0,
            buffer: std::ptr::null(),
        };
        let rendered = unsafe {
            ft_bench_render(self.bench.as_ptr(), u32::from(glyph.0), offset, &mut bitmap)
        };
        (rendered != 0).then_some(bitmap)
    }

    pub fn render_all(&mut self, glyphs: &[GlyphId], ppem: f32) -> usize {
        self.sized(ppem);
        glyphs
            .iter()
            .enumerate()
            .filter_map(|(index, &glyph)| self.bitmap(glyph, subpixel_offset(index)))
            .map(|bitmap| (bitmap.width * bitmap.rows) as usize)
            .sum()
    }

    pub fn mask(&mut self, glyph: GlyphId, ppem: f32, offset: f32) -> Mask {
        self.sized(ppem);
        let Some(bitmap) = self.bitmap(glyph, offset) else {
            return Mask::default();
        };
        let mut pixels = Vec::with_capacity((bitmap.width * bitmap.rows) as usize);
        for row in 0..bitmap.rows as isize {
            let start = unsafe { bitmap.buffer.offset(row * bitmap.pitch as isize) };
            let line = unsafe { std::slice::from_raw_parts(start, bitmap.width as usize) };
            pixels.extend_from_slice(line);
        }
        Mask {
            left: bitmap.left,
            top: bitmap.top,
            width: bitmap.width,
            height: bitmap.rows,
            pixels,
        }
    }

    pub fn outline_segments(&mut self, glyph: GlyphId) -> usize {
        let segments = unsafe { ft_bench_outline(self.bench.as_ptr(), u32::from(glyph.0)) };
        usize::try_from(segments).unwrap_or(0)
    }
}

impl Drop for FreeType {
    fn drop(&mut self) {
        unsafe { ft_bench_close(self.bench.as_ptr()) };
    }
}

impl std::fmt::Debug for FreeType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("FreeType").finish_non_exhaustive()
    }
}
