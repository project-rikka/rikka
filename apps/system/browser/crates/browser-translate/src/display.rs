use rikka_types::Choice;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Display {
    #[default]
    Replace,
    Bilingual,
}

impl Choice for Display {
    const ALL: &'static [Display] = &[Display::Replace, Display::Bilingual];

    fn slug(self) -> &'static str {
        match self {
            Display::Replace => "replace",
            Display::Bilingual => "bilingual",
        }
    }
}
