use rikka_input::Chord;

use crate::text::Label;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TooltipSide {
    #[default]
    Below,
    Above,
    Before,
    After,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tooltip {
    label: Label,
    shortcut: Option<Chord>,
    side: TooltipSide,
}

impl Tooltip {
    pub fn new(label: impl Into<Label>) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
            side: TooltipSide::default(),
        }
    }

    #[must_use]
    pub fn shortcut(mut self, chord: impl Into<Option<Chord>>) -> Self {
        self.shortcut = chord.into();
        self
    }

    #[must_use]
    pub fn side(mut self, side: TooltipSide) -> Self {
        self.side = side;
        self
    }

    pub(crate) fn label(&self) -> &str {
        self.label.as_str()
    }

    pub(crate) fn chord(&self) -> Option<Chord> {
        self.shortcut
    }

    pub(crate) fn placement(&self) -> TooltipSide {
        self.side
    }

    pub(crate) fn is_blank(&self) -> bool {
        self.label.as_str().trim().is_empty()
    }
}

impl From<Label> for Tooltip {
    fn from(label: Label) -> Self {
        Self::new(label)
    }
}

impl From<&str> for Tooltip {
    fn from(label: &str) -> Self {
        Self::new(label)
    }
}

impl From<String> for Tooltip {
    fn from(label: String) -> Self {
        Self::new(label)
    }
}
