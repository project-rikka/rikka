#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    #[must_use]
    pub fn offset(self, outer: (u32, u32), inner: (u32, u32)) -> (i64, i64) {
        let slack = (
            i64::from(outer.0) - i64::from(inner.0),
            i64::from(outer.1) - i64::from(inner.1),
        );
        (
            self.horizontal_fraction().apply(slack.0),
            self.vertical_fraction().apply(slack.1),
        )
    }

    const fn horizontal_fraction(self) -> Fraction {
        match self {
            Anchor::TopLeft | Anchor::Left | Anchor::BottomLeft => Fraction::Start,
            Anchor::Top | Anchor::Center | Anchor::Bottom => Fraction::Middle,
            Anchor::TopRight | Anchor::Right | Anchor::BottomRight => Fraction::End,
        }
    }

    const fn vertical_fraction(self) -> Fraction {
        match self {
            Anchor::TopLeft | Anchor::Top | Anchor::TopRight => Fraction::Start,
            Anchor::Left | Anchor::Center | Anchor::Right => Fraction::Middle,
            Anchor::BottomLeft | Anchor::Bottom | Anchor::BottomRight => Fraction::End,
        }
    }
}

enum Fraction {
    Start,
    Middle,
    End,
}

impl Fraction {
    const fn apply(self, slack: i64) -> i64 {
        match self {
            Fraction::Start => 0,
            Fraction::Middle => slack / 2,
            Fraction::End => slack,
        }
    }
}
