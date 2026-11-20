use cut_project::{Incoming, Outgoing, Smooth, Timing};
use rikka_timeline::{KeyGlyph, KeyShape};

pub(crate) const SUMMARY: KeyGlyph = KeyGlyph::whole(KeyShape::Diamond);

pub(crate) fn of_timing(timing: Timing) -> KeyGlyph {
    let bent = match timing.smooth {
        Smooth::Auto => KeyShape::Circle,
        Smooth::Manual | Smooth::Continuous => KeyShape::Hourglass,
    };
    KeyGlyph {
        incoming: match timing.incoming {
            Incoming::Linear => KeyShape::Diamond,
            Incoming::Bezier(_) => bent,
        },
        outgoing: match timing.outgoing {
            Outgoing::Hold => KeyShape::Square,
            Outgoing::Linear => KeyShape::Diamond,
            Outgoing::Bezier(_) => bent,
            Outgoing::Preset(_) => KeyShape::Hourglass,
        },
    }
}
