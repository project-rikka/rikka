use rikka_types::Size;

use crate::Anchor;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Span {
    Fixed(f32),
    Stretched,
}

impl Span {
    #[must_use]
    fn requested(self) -> f32 {
        match self {
            Span::Fixed(length) => length,
            Span::Stretched => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extent {
    pub(crate) width: Span,
    pub(crate) height: Span,
}

impl Extent {
    pub(crate) const FILL: Extent = Extent {
        width: Span::Stretched,
        height: Span::Stretched,
    };

    #[must_use]
    pub(crate) fn fixed(size: Size) -> Extent {
        Extent {
            width: Span::Fixed(size.width),
            height: Span::Fixed(size.height),
        }
    }

    #[must_use]
    pub fn requested(self) -> Size {
        Size::new(self.width.requested(), self.height.requested())
    }

    #[must_use]
    pub fn anchored(self, anchor: Anchor) -> Anchor {
        let mut anchored = anchor;
        if self.width == Span::Stretched {
            anchored |= Anchor::LEFT | Anchor::RIGHT;
        }
        if self.height == Span::Stretched {
            anchored |= Anchor::TOP | Anchor::BOTTOM;
        }
        anchored
    }

    pub(super) fn empty_axis(self) -> Option<Axis> {
        let empty = |span: Span| matches!(span, Span::Fixed(length) if length.round() < 1.0);
        if empty(self.width) {
            Some(Axis::Width)
        } else if empty(self.height) {
            Some(Axis::Height)
        } else {
            None
        }
    }
}

impl Default for Extent {
    fn default() -> Extent {
        Extent::FILL
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Width,
    Height,
}

impl std::fmt::Display for Axis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Axis::Width => "width",
            Axis::Height => "height",
        })
    }
}
