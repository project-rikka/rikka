use rikka_ffmpeg::avutil::{self, AVPixelFormat};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelLayout {
    Rgba,
    Bgra,
}

impl PixelLayout {
    pub fn pixel_format(self) -> AVPixelFormat {
        match self {
            Self::Rgba => avutil::AV_PIX_FMT_RGBA,
            Self::Bgra => avutil::AV_PIX_FMT_BGRA,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Picture<'a> {
    pub bytes: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub layout: PixelLayout,
}
