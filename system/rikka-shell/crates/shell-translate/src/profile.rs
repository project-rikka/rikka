use rikka_types::Choice;
use rikka_ui::{Label, tr};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Profile {
    #[default]
    Natural,
    Casual,
    Polite,
    Business,
}

impl Choice for Profile {
    const ALL: &'static [Profile] = &[
        Profile::Natural,
        Profile::Casual,
        Profile::Polite,
        Profile::Business,
    ];

    fn slug(self) -> &'static str {
        match self {
            Profile::Natural => "natural",
            Profile::Casual => "casual",
            Profile::Polite => "polite",
            Profile::Business => "business",
        }
    }
}

impl Profile {
    #[must_use]
    pub(crate) fn label(self) -> Label {
        match self {
            Profile::Natural => tr!("shell.translate.profile.natural"),
            Profile::Casual => tr!("shell.translate.profile.casual"),
            Profile::Polite => tr!("shell.translate.profile.polite"),
            Profile::Business => tr!("shell.translate.profile.business"),
        }
    }

    #[must_use]
    pub(crate) fn listed(index: usize) -> Option<Profile> {
        Profile::ALL.get(index).copied()
    }

    #[must_use]
    pub(crate) fn at(self) -> Option<usize> {
        Profile::ALL.iter().position(|offered| *offered == self)
    }

    pub(crate) fn labels() -> impl Iterator<Item = Label> {
        Profile::ALL.iter().copied().map(Profile::label)
    }
}
