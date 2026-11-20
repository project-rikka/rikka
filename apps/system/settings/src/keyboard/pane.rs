use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Pane {
    #[default]
    Input,
    Shortcuts,
}

impl Pane {
    pub(crate) const ALL: [Self; 2] = [Self::Input, Self::Shortcuts];

    pub(crate) fn title(self) -> Label {
        match self {
            Self::Input => tr!("settings.keyboard.pane.input"),
            Self::Shortcuts => tr!("settings.keyboard.pane.shortcuts"),
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
