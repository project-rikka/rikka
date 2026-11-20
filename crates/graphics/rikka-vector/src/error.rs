#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("path exceeds the rasterizer limits")]
    TooComplex,
    #[error("path has more points than a path may hold")]
    Truncated,
    #[error("invalid {width}x{height} surface")]
    InvalidSize { width: u32, height: u32 },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
