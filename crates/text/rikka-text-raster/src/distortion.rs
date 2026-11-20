use rikka_text_font::{Point, Synthesis, Transform};

const SYNTHETIC_OBLIQUE_SKEW: f32 = 0.25;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Distortion {
    pub(crate) embolden: f32,
    pub(crate) skew: f32,
}

impl Distortion {
    pub(crate) fn new(synthesis: Synthesis, units_per_em: f32) -> Self {
        Self {
            embolden: synthesis.extra_advance_em() * 0.5 * units_per_em,
            skew: if synthesis.oblique {
                SYNTHETIC_OBLIQUE_SKEW
            } else {
                0.0
            },
        }
    }

    pub(crate) fn placement(self, scale: f32, offset: Point) -> Transform {
        Transform {
            xx: scale,
            yx: 0.0,
            xy: scale * self.skew,
            yy: scale,
            dx: self.embolden * scale + offset.x,
            dy: offset.y,
        }
    }
}
