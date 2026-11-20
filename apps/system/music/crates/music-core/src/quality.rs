#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Quality {
    Low,
    Standard,
    #[default]
    Highest,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Low, Quality::Standard, Quality::Highest];

    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Quality::Low => "low",
            Quality::Standard => "standard",
            Quality::Highest => "highest",
        }
    }

    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Quality> {
        Quality::ALL
            .into_iter()
            .find(|quality| quality.slug() == slug)
    }
}
