use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tab {
    #[default]
    Processes,
    Performance,
    Details,
}

impl Tab {
    pub(crate) const ALL: [Self; 3] = [Self::Processes, Self::Performance, Self::Details];

    pub(crate) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|tab| *tab == self)
            .expect("every tab is in ALL")
    }

    fn title(self) -> Label {
        match self {
            Self::Processes => tr!("tasks.tab.processes"),
            Self::Performance => tr!("tasks.tab.performance"),
            Self::Details => tr!("tasks.tab.details"),
        }
    }

    fn symbol(self) -> MaterialSymbol {
        match self {
            Self::Processes => MaterialSymbol::LIST_ALT,
            Self::Performance => MaterialSymbol::MONITORING,
            Self::Details => MaterialSymbol::TABLE_ROWS,
        }
    }

    pub(crate) fn nav_item(self) -> NavItem {
        nav_item(self.title()).icon(self.symbol())
    }
}
