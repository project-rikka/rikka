use super::picture::Resolution;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StillFormat {
    #[default]
    Png,
    Jpeg,
}

impl StillFormat {
    pub const ALL: [StillFormat; 2] = [StillFormat::Png, StillFormat::Jpeg];

    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            StillFormat::Png => "png",
            StillFormat::Jpeg => "jpg",
        }
    }

    #[must_use]
    pub fn mime_type(self) -> &'static str {
        match self {
            StillFormat::Png => "image/png",
            StillFormat::Jpeg => "image/jpeg",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Still {
    pub format: StillFormat,
    pub resolution: Resolution,
}
