use super::class::{Class, Classes};

use Class as L;

pub(crate) const ATTACHING: Classes = Classes::new(&[L::CombiningMark, L::Zwj]);
pub(crate) const NOT_ABSORBING: Classes = Classes::new(&[
    L::MandatoryBreak,
    L::CarriageReturn,
    L::LineFeed,
    L::NextLine,
    L::Space,
    L::ZWSpace,
]);
const AKSARAS: Classes = Classes::new(&[L::Aksara, L::AksaraStart]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Unit {
    pub(crate) complex: bool,
    pub(crate) class: Class,
    pub(crate) east_asian: bool,
    pub(crate) initial_quote: bool,
    pub(crate) final_quote: bool,
    pub(crate) unassigned_pictographic: bool,
    pub(crate) dotted_circle: bool,
    pub(crate) joiner_end: bool,
}

impl Unit {
    pub(crate) fn normalized(self) -> Self {
        if ATTACHING.has(self.class) {
            return Self {
                complex: self.complex,
                ..Self::plain(self.class)
            };
        }
        let quote = self.class == L::Quotation;
        Self {
            initial_quote: quote && self.initial_quote,
            final_quote: quote && self.final_quote,
            joiner_end: false,
            ..self
        }
    }

    pub(crate) fn plain(class: Class) -> Self {
        Self {
            complex: false,
            class,
            east_asian: false,
            initial_quote: false,
            final_quote: false,
            unassigned_pictographic: false,
            dotted_circle: false,
            joiner_end: false,
        }
    }

    pub(crate) fn as_base(self) -> Self {
        if self.attaches() {
            Self {
                joiner_end: self.class == L::Zwj,
                ..Self::plain(L::Alphabetic)
            }
        } else {
            Self {
                complex: false,
                ..self
            }
        }
    }

    pub(crate) fn attaches(self) -> bool {
        ATTACHING.has(self.class)
    }

    pub(crate) fn absorbs(self) -> bool {
        !NOT_ABSORBING.has(self.class)
    }

    pub(crate) fn is_aksara(self) -> bool {
        AKSARAS.has(self.class) || self.dotted_circle
    }
}
