use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mark {
    #[default]
    Available,
    Installed,
    Adding,
    Removing,
}

impl Mark {
    pub(crate) fn is_included(self) -> bool {
        matches!(self, Self::Installed | Self::Adding)
    }

    pub(crate) fn is_pending(self) -> bool {
        matches!(self, Self::Adding | Self::Removing)
    }

    pub(crate) fn symbol(self) -> MaterialSymbol {
        match self {
            Self::Available => MaterialSymbol::DEPLOYED_CODE,
            Self::Installed => MaterialSymbol::CHECK_CIRCLE,
            Self::Adding => MaterialSymbol::ADD_CIRCLE,
            Self::Removing => MaterialSymbol::DO_NOT_DISTURB_ON,
        }
    }

    pub(crate) fn tint(self, palette: &Palette) -> Color {
        match self {
            Self::Available => palette.text_muted,
            Self::Installed => palette.success,
            Self::Adding => palette.accent,
            Self::Removing => palette.danger,
        }
    }

    pub(crate) fn label(self) -> Option<Label> {
        match self {
            Self::Available => None,
            Self::Installed => Some(tr!("packages.state.installed")),
            Self::Adding => Some(tr!("packages.state.adding")),
            Self::Removing => Some(tr!("packages.state.removing")),
        }
    }
}
