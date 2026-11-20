use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tab {
    #[default]
    Threads,
    Files,
    Git,
}

impl Tab {
    pub(crate) const ALL: [Tab; 3] = [Tab::Threads, Tab::Files, Tab::Git];

    pub(crate) fn at(self) -> usize {
        Tab::ALL.iter().position(|tab| *tab == self).unwrap_or(0)
    }

    pub(crate) fn label(self) -> Label {
        match self {
            Tab::Threads => tr!("ade.sidebar.threads"),
            Tab::Files => tr!("ade.sidebar.files"),
            Tab::Git => tr!("ade.sidebar.git"),
        }
    }

    pub(crate) fn glyph(self) -> MaterialSymbol {
        match self {
            Tab::Threads => MaterialSymbol::CHAT_BUBBLE,
            Tab::Files => MaterialSymbol::FOLDER,
            Tab::Git => MaterialSymbol::ACCOUNT_TREE,
        }
    }
}
