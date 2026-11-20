use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Section {
    #[default]
    Search,
    Installed,
    Apps,
    Generations,
}

impl Section {
    pub(crate) const ALL: [Self; 4] =
        [Self::Search, Self::Installed, Self::Apps, Self::Generations];

    pub(crate) fn at(self) -> usize {
        Self::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or_default()
    }

    fn title(self) -> Label {
        match self {
            Self::Search => tr!("packages.section.search"),
            Self::Installed => tr!("packages.section.installed"),
            Self::Apps => tr!("packages.section.apps"),
            Self::Generations => tr!("packages.section.generations"),
        }
    }

    pub(crate) fn empty(self) -> Label {
        match self {
            Self::Search => tr!("packages.empty.catalog"),
            Self::Installed => tr!("packages.empty.installed"),
            Self::Apps => tr!("packages.empty.apps"),
            Self::Generations => tr!("packages.empty.generations"),
        }
    }

    pub(crate) fn symbol(self) -> MaterialSymbol {
        match self {
            Self::Search => MaterialSymbol::SEARCH,
            Self::Installed => MaterialSymbol::CHECK_CIRCLE,
            Self::Apps => MaterialSymbol::APPS,
            Self::Generations => MaterialSymbol::HISTORY,
        }
    }

    pub(crate) fn nav_item(self) -> NavItem {
        nav_item(self.title()).icon(self.symbol())
    }
}
