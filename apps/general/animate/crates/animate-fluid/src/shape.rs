#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Stream,
    Shower,
    Drip,
    Blob { size: f32 },
    Surface { figure: String, materials: Vec<u32> },
}

impl Shape {
    pub const BLOB_SIZES: std::ops::RangeInclusive<f32> = 0.01..=0.5;
    pub const BLOB_SIZE: f32 = 0.04;

    #[must_use]
    pub fn blob() -> Self {
        Self::Blob {
            size: Self::BLOB_SIZE,
        }
    }

    #[must_use]
    pub fn surface(figure: impl Into<String>, materials: impl IntoIterator<Item = u32>) -> Self {
        let mut materials: Vec<u32> = materials.into_iter().collect();
        materials.sort_unstable();
        materials.dedup();
        Self::Surface {
            figure: figure.into(),
            materials,
        }
    }

    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Stream => "stream",
            Self::Shower => "shower",
            Self::Drip => "drip",
            Self::Blob { .. } => "blob",
            Self::Surface { .. } => "surface",
        }
    }

    #[must_use]
    pub fn aims(&self) -> bool {
        matches!(self, Self::Stream | Self::Shower | Self::Blob { .. })
    }

    #[must_use]
    pub fn flows(&self) -> bool {
        !matches!(self, Self::Blob { .. })
    }

    #[must_use]
    pub fn launches(&self) -> bool {
        !matches!(self, Self::Surface { .. })
    }
}
