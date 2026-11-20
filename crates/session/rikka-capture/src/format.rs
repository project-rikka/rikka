use rikka_wayland_client::wl_shm;

const fn fourcc(code: [u8; 4]) -> u32 {
    (code[0] as u32) | (code[1] as u32) << 8 | (code[2] as u32) << 16 | (code[3] as u32) << 24
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    Xrgb8888,
    Argb8888,
    Xbgr8888,
    Abgr8888,
}

impl PixelFormat {
    pub const ALL: [PixelFormat; 4] = [
        PixelFormat::Xbgr8888,
        PixelFormat::Abgr8888,
        PixelFormat::Xrgb8888,
        PixelFormat::Argb8888,
    ];

    #[must_use]
    pub fn fourcc(self) -> u32 {
        match self {
            PixelFormat::Xrgb8888 => fourcc(*b"XR24"),
            PixelFormat::Argb8888 => fourcc(*b"AR24"),
            PixelFormat::Xbgr8888 => fourcc(*b"XB24"),
            PixelFormat::Abgr8888 => fourcc(*b"AB24"),
        }
    }

    #[must_use]
    pub fn from_fourcc(code: u32) -> Option<PixelFormat> {
        PixelFormat::ALL
            .into_iter()
            .find(|format| format.fourcc() == code)
    }

    #[must_use]
    pub fn is_bgr(self) -> bool {
        matches!(self, PixelFormat::Xrgb8888 | PixelFormat::Argb8888)
    }

    #[must_use]
    pub fn has_alpha(self) -> bool {
        matches!(self, PixelFormat::Argb8888 | PixelFormat::Abgr8888)
    }

    pub(crate) fn from_shm(format: wl_shm::Format) -> Option<PixelFormat> {
        match format {
            wl_shm::Format::Xrgb8888 => Some(PixelFormat::Xrgb8888),
            wl_shm::Format::Argb8888 => Some(PixelFormat::Argb8888),
            wl_shm::Format::Xbgr8888 => Some(PixelFormat::Xbgr8888),
            wl_shm::Format::Abgr8888 => Some(PixelFormat::Abgr8888),
            _ => None,
        }
    }

    pub(crate) fn to_shm(self) -> wl_shm::Format {
        match self {
            PixelFormat::Xrgb8888 => wl_shm::Format::Xrgb8888,
            PixelFormat::Argb8888 => wl_shm::Format::Argb8888,
            PixelFormat::Xbgr8888 => wl_shm::Format::Xbgr8888,
            PixelFormat::Abgr8888 => wl_shm::Format::Abgr8888,
        }
    }
}
