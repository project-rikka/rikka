#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Quality {
    #[default]
    Auto,
    Full,
    Half,
    Quarter,
}

impl Quality {
    pub(crate) const ALL: [Quality; 4] = [
        Quality::Auto,
        Quality::Full,
        Quality::Half,
        Quality::Quarter,
    ];

    pub(crate) fn scale(self, display_scale: f32) -> f32 {
        let selected: f32 = match self {
            Quality::Auto => return display_scale,
            Quality::Full => 1.0,
            Quality::Half => 0.5,
            Quality::Quarter => 0.25,
        };
        selected.min(display_scale)
    }
}
