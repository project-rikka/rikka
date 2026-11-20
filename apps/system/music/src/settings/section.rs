use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Section {
    #[default]
    Accounts,
    Library,
    Output,
    Quality,
    Sources,
    Storage,
}

impl Section {
    pub(crate) const ALL: [Section; 6] = [
        Section::Accounts,
        Section::Library,
        Section::Output,
        Section::Quality,
        Section::Sources,
        Section::Storage,
    ];

    pub(crate) fn from_index(index: usize) -> Section {
        Section::ALL.get(index).copied().unwrap_or_default()
    }

    pub(crate) fn index(self) -> usize {
        Section::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or_default()
    }

    pub(crate) fn title(self) -> Label {
        match self {
            Section::Accounts => tr!("music.settings.accounts"),
            Section::Library => tr!("music.settings.library"),
            Section::Output => tr!("music.settings.output"),
            Section::Quality => tr!("music.settings.quality"),
            Section::Sources => tr!("music.settings.sources"),
            Section::Storage => tr!("music.settings.storage"),
        }
    }

    fn symbol(self) -> MaterialSymbol {
        match self {
            Section::Accounts => MaterialSymbol::MANAGE_ACCOUNTS,
            Section::Library => MaterialSymbol::LIBRARY_MUSIC,
            Section::Output => MaterialSymbol::SPEAKER,
            Section::Quality => MaterialSymbol::HIGH_QUALITY,
            Section::Sources => MaterialSymbol::SWAP_VERT,
            Section::Storage => MaterialSymbol::DATABASE,
        }
    }

    pub(crate) fn nav_item(self) -> NavItem {
        nav_item(self.title()).icon(self.symbol())
    }
}
