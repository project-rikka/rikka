use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Trimmer {
    #[default]
    Select,
    Ripple,
    Roll,
    Slip,
    Slide,
}

impl Trimmer {
    pub(crate) const ALL: [Trimmer; 5] = [
        Trimmer::Select,
        Trimmer::Ripple,
        Trimmer::Roll,
        Trimmer::Slip,
        Trimmer::Slide,
    ];

    pub(crate) fn symbol(self) -> MaterialSymbol {
        match self {
            Trimmer::Select => MaterialSymbol::ARROW_SELECTOR_TOOL,
            Trimmer::Ripple => MaterialSymbol::KEYBOARD_TAB,
            Trimmer::Roll => MaterialSymbol::ARROW_RANGE,
            Trimmer::Slip => MaterialSymbol::SYNC_ALT,
            Trimmer::Slide => MaterialSymbol::HORIZONTAL_DISTRIBUTE,
        }
    }
}
