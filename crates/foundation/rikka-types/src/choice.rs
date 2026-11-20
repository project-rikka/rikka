pub trait Choice: Copy + Default + PartialEq + 'static {
    const ALL: &'static [Self];

    fn slug(self) -> &'static str;

    #[must_use]
    fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|choice| choice.slug() == slug)
    }

    #[must_use]
    fn of_slug(slug: &str) -> Self {
        Self::from_slug(slug).unwrap_or_default()
    }
}
