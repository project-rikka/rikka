use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Avif,
    Heif,
    Tiff,
    OpenExr,
    Jpeg2000,
    JpegXl,
}

impl Format {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Format::Avif => "AVIF",
            Format::Heif => "HEIF",
            Format::Tiff => "TIFF",
            Format::OpenExr => "OpenEXR",
            Format::Jpeg2000 => "JPEG 2000",
            Format::JpegXl => "JPEG XL",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
