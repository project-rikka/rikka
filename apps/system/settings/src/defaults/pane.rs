use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Pane {
    #[default]
    Apps,
    Files,
    Links,
}

impl Pane {
    pub(crate) const ALL: [Self; 3] = [Self::Apps, Self::Files, Self::Links];

    pub(crate) fn title(self) -> Label {
        match self {
            Self::Apps => tr!("settings.defaults.pane.apps"),
            Self::Files => tr!("settings.defaults.pane.files"),
            Self::Links => tr!("settings.defaults.pane.links"),
        }
    }

    pub(crate) fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or_default()
    }

    pub(crate) fn index(self) -> usize {
        Self::ALL
            .into_iter()
            .position(|pane| pane == self)
            .unwrap_or_default()
    }
}
