use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Showing {
    #[default]
    Changes,
    History,
}

impl Showing {
    pub(crate) const ALL: [Showing; 2] = [Showing::Changes, Showing::History];

    pub(crate) fn at(self) -> usize {
        Showing::ALL
            .iter()
            .position(|shown| *shown == self)
            .unwrap_or(0)
    }

    pub(crate) fn label(self) -> Label {
        match self {
            Showing::Changes => tr!("ade.git.changes"),
            Showing::History => tr!("ade.git.history"),
        }
    }

    pub(crate) fn glyph(self) -> MaterialSymbol {
        match self {
            Showing::Changes => MaterialSymbol::EDIT_DOCUMENT,
            Showing::History => MaterialSymbol::HISTORY,
        }
    }
}
