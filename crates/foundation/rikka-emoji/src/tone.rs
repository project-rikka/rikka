use rikka_types::Choice;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Plain,
    Light,
    MediumLight,
    Medium,
    MediumDark,
    Dark,
}

impl Tone {
    pub const SAMPLE: &str = "✋";

    pub(crate) fn slot(self) -> Option<usize> {
        match self {
            Tone::Plain => None,
            Tone::Light => Some(0),
            Tone::MediumLight => Some(1),
            Tone::Medium => Some(2),
            Tone::MediumDark => Some(3),
            Tone::Dark => Some(4),
        }
    }
}

impl Choice for Tone {
    const ALL: &'static [Tone] = &[
        Tone::Plain,
        Tone::Light,
        Tone::MediumLight,
        Tone::Medium,
        Tone::MediumDark,
        Tone::Dark,
    ];

    fn slug(self) -> &'static str {
        match self {
            Tone::Plain => "plain",
            Tone::Light => "light",
            Tone::MediumLight => "medium-light",
            Tone::Medium => "medium",
            Tone::MediumDark => "medium-dark",
            Tone::Dark => "dark",
        }
    }
}
