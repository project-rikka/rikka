use rikka_emoji::Group;
use rikka_ui::{Label, MaterialSymbol, tr};

use crate::group;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Tab {
    Recent,
    Group(Group),
}

impl Tab {
    #[must_use]
    pub(crate) fn all() -> [Tab; Group::ALL.len() + 1] {
        let mut all = [Tab::Recent; Group::ALL.len() + 1];
        all[1..].copy_from_slice(&Group::ALL.map(Tab::Group));
        all
    }

    #[must_use]
    pub(crate) fn label(self) -> Label {
        match self {
            Tab::Recent => tr!("shell.emoji.recent"),
            Tab::Group(shown) => group::label(shown),
        }
    }

    #[must_use]
    pub(crate) fn symbol(self) -> MaterialSymbol {
        match self {
            Tab::Recent => MaterialSymbol::HISTORY,
            Tab::Group(shown) => group::symbol(shown),
        }
    }
}
