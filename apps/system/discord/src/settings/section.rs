use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Section {
    #[default]
    Accounts,
    Voice,
    Notifications,
    Appearance,
}

impl Section {
    pub(crate) const ALL: [Section; 4] = [
        Section::Accounts,
        Section::Voice,
        Section::Notifications,
        Section::Appearance,
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
            Section::Accounts => tr!("discord.settings.accounts"),
            Section::Voice => tr!("discord.settings.voice"),
            Section::Notifications => tr!("discord.settings.notifications"),
            Section::Appearance => tr!("discord.settings.appearance"),
        }
    }

    fn symbol(self) -> MaterialSymbol {
        match self {
            Section::Accounts => MaterialSymbol::MANAGE_ACCOUNTS,
            Section::Voice => MaterialSymbol::HEADSET_MIC,
            Section::Notifications => MaterialSymbol::NOTIFICATIONS,
            Section::Appearance => MaterialSymbol::PALETTE,
        }
    }

    pub(crate) fn nav_item(self) -> NavItem {
        nav_item(self.title()).icon(self.symbol())
    }
}
