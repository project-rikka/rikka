use std::f64::consts::{LN_2, PI};

const OVERSHOOT: f64 = 1.701_58;
const BALANCED: f64 = OVERSHOOT * 1.525;
const EXPO_FLOOR: f64 = 1.0 / 1024.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Linear,
    Sine,
    Quad,
    Cubic,
    Quart,
    Quint,
    Expo,
    Circ,
    Back,
    Elastic,
    Bounce,
}

impl Family {
    pub const ALL: [Family; 11] = [
        Family::Linear,
        Family::Sine,
        Family::Quad,
        Family::Cubic,
        Family::Quart,
        Family::Quint,
        Family::Expo,
        Family::Circ,
        Family::Back,
        Family::Elastic,
        Family::Bounce,
    ];

    #[must_use]
    pub fn overshoots(self) -> bool {
        matches!(self, Family::Back | Family::Elastic)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ease {
    In,
    Out,
    InOut,
}

impl Ease {
    pub const ALL: [Ease; 3] = [Ease::In, Ease::Out, Ease::InOut];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Easing {
    pub family: Family,
    pub ease: Ease,
}

impl Easing {
    pub const LINEAR: Easing = Easing::new(Family::Linear, Ease::InOut);

    #[must_use]
    pub const fn new(family: Family, ease: Ease) -> Easing {
        Easing { family, ease }
    }

    #[must_use]
    pub fn reversed(self) -> Easing {
        let ease = match self.ease {
            Ease::In => Ease::Out,
            Ease::Out => Ease::In,
            Ease::InOut => Ease::InOut,
        };
        Easing { ease, ..self }
    }

    #[must_use]
    pub fn at(self, progress: f64) -> f64 {
        let x = progress.clamp(0.0, 1.0);
        let rising = |x: f64| self.entering(x);
        match self.ease {
            Ease::In => rising(x),
            Ease::Out => 1.0 - rising(1.0 - x),
            Ease::InOut if x < 0.5 => rising(2.0 * x) / 2.0,
            Ease::InOut => 1.0 - rising(2.0 - 2.0 * x) / 2.0,
        }
    }

    pub(crate) fn entering_slopes(self) -> Option<(f64, f64)> {
        Some(match self.family {
            Family::Linear => (1.0, 1.0),
            Family::Sine => (0.0, PI / 2.0),
            Family::Quad => (0.0, 2.0),
            Family::Cubic => (0.0, 3.0),
            Family::Quart => (0.0, 4.0),
            Family::Quint => (0.0, 5.0),
            Family::Expo => (10.0 * LN_2 * EXPO_FLOOR, 10.0 * LN_2),
            Family::Circ => (0.0, f64::INFINITY),
            Family::Back => (0.0, self.overshoot() + 3.0),
            Family::Elastic | Family::Bounce => return None,
        })
    }

    fn overshoot(self) -> f64 {
        match self.ease {
            Ease::InOut => BALANCED,
            Ease::In | Ease::Out => OVERSHOOT,
        }
    }

    fn entering(self, x: f64) -> f64 {
        match self.family {
            Family::Linear => x,
            Family::Sine => 1.0 - (x * PI / 2.0).cos(),
            Family::Quad => x.powi(2),
            Family::Cubic => x.powi(3),
            Family::Quart => x.powi(4),
            Family::Quint => x.powi(5),
            Family::Expo if x <= 0.0 => 0.0,
            Family::Expo => 2f64.powf(10.0 * x - 10.0),
            Family::Circ => 1.0 - (1.0 - x * x).max(0.0).sqrt(),
            Family::Back => {
                let overshoot = self.overshoot();
                (overshoot + 1.0) * x.powi(3) - overshoot * x.powi(2)
            }
            Family::Elastic if x <= 0.0 => 0.0,
            Family::Elastic if x >= 1.0 => 1.0,
            Family::Elastic => match self.ease {
                Ease::InOut => {
                    -(2f64.powf(10.0 * x - 10.0)) * ((10.0 * x - 11.125) * (2.0 * PI / 4.5)).sin()
                }
                Ease::In | Ease::Out => {
                    -(2f64.powf(10.0 * x - 10.0)) * ((10.0 * x - 10.75) * (2.0 * PI / 3.0)).sin()
                }
            },
            Family::Bounce => 1.0 - bounced(1.0 - x),
        }
    }
}

fn bounced(x: f64) -> f64 {
    const SPRING: f64 = 7.5625;
    const SPAN: f64 = 2.75;
    match x {
        x if x < 1.0 / SPAN => SPRING * x * x,
        x if x < 2.0 / SPAN => SPRING * (x - 1.5 / SPAN).powi(2) + 0.75,
        x if x < 2.5 / SPAN => SPRING * (x - 2.25 / SPAN).powi(2) + 0.9375,
        x => SPRING * (x - 2.625 / SPAN).powi(2) + 0.984_375,
    }
}
