use crate::entry::Entry;
use crate::format;

#[derive(Clone, Debug)]
pub(crate) enum Operand {
    Entry(Entry),
    Value(f64),
}

impl Operand {
    pub(crate) fn zero() -> Self {
        Self::Entry(Entry::zero())
    }

    pub(crate) fn value(&self) -> f64 {
        match self {
            Self::Entry(entry) => entry.value(),
            Self::Value(value) => *value,
        }
    }

    pub(crate) fn text(&self) -> String {
        match self {
            Self::Entry(entry) => entry.text(),
            Self::Value(value) => format::number(*value),
        }
    }

    pub(crate) fn push_digit(&mut self, digit: u8) {
        self.entry_mut().push_digit(digit);
    }

    pub(crate) fn push_point(&mut self) {
        self.entry_mut().push_point();
    }

    pub(crate) fn backspace(&mut self) {
        if let Self::Entry(entry) = self {
            entry.backspace();
        }
    }

    pub(crate) fn negate_typed(&mut self) -> bool {
        match self {
            Self::Entry(entry) => {
                entry.negate();
                true
            }
            Self::Value(_) => false,
        }
    }

    fn entry_mut(&mut self) -> &mut Entry {
        if let Self::Value(_) = self {
            *self = Self::zero();
        }
        let Self::Entry(entry) = self else {
            unreachable!("operand was just reset to an entry")
        };
        entry
    }
}
