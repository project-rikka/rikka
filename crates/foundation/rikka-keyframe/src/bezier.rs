use crate::easing::{Ease, Easing, Family};

const SAMPLES: usize = 64;
const PLANAR_GRID: usize = 48;
const LINEAR_GRID: usize = 512;
const FINEST: f64 = 1e-6;
const NEWTON_STEPS: usize = 8;
const HALVINGS: usize = 48;
const FLAT: f64 = 1e-9;
const LONGEST: f64 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl CubicBezier {
    pub const LINEAR: CubicBezier = CubicBezier::new(0.0, 0.0, 1.0, 1.0);
    pub const EASE: CubicBezier = CubicBezier::new(0.25, 0.1, 0.25, 1.0);
    pub const EASE_IN: CubicBezier = CubicBezier::new(0.42, 0.0, 1.0, 1.0);
    pub const EASE_OUT: CubicBezier = CubicBezier::new(0.0, 0.0, 0.58, 1.0);
    pub const EASE_IN_OUT: CubicBezier = CubicBezier::new(0.42, 0.0, 0.58, 1.0);

    #[must_use]
    pub const fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> CubicBezier {
        CubicBezier { x1, y1, x2, y2 }
    }

    #[must_use]
    pub fn at(self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        cubic(self.y1, self.y2, self.parameter_at(x))
    }

    #[must_use]
    pub fn reflected(self) -> CubicBezier {
        CubicBezier::new(1.0 - self.x2, 1.0 - self.y2, 1.0 - self.x1, 1.0 - self.y1)
    }

    #[must_use]
    pub fn fit(easing: Easing) -> Option<CubicBezier> {
        let (start, end) = easing.entering_slopes()?;
        if easing.family == Family::Linear {
            return Some(CubicBezier::LINEAR);
        }
        let bounded = !easing.family.overshoots();
        let (leaving, arriving) = (Tangent::of(start, bounded), Tangent::of(end, bounded));
        Some(match easing.ease {
            Ease::In => {
                let shaped = |[out, back]: [f64; 2]| {
                    let (first, second) = (leaving.at(out), arriving.at(back));
                    CubicBezier::new(first.0, first.1, 1.0 - second.0, 1.0 - second.1)
                };
                shaped(closest(PLANAR_GRID, |lengths| {
                    largest_error(shaped(lengths), easing)
                }))
            }
            Ease::Out => CubicBezier::fit(Easing::new(easing.family, Ease::In))?.reflected(),
            Ease::InOut if bounded => {
                let shaped = |[length]: [f64; 1]| {
                    let handle = leaving.at(length);
                    CubicBezier::new(handle.0, handle.1, 1.0 - handle.0, 1.0 - handle.1)
                };
                shaped(closest(LINEAR_GRID, |length| {
                    largest_error(shaped(length), easing)
                }))
            }
            Ease::InOut => {
                let shaped = |[across, rise]: [f64; 2]| {
                    let lift = (rise - 0.5) * LONGEST;
                    CubicBezier::new(across, lift, 1.0 - across, 1.0 - lift)
                };
                shaped(closest(PLANAR_GRID, |handle| {
                    largest_error(shaped(handle), easing)
                }))
            }
        })
    }

    fn parameter_at(self, x: f64) -> f64 {
        let mut t = x;
        for _ in 0..NEWTON_STEPS {
            let slope = cubic_slope(self.x1, self.x2, t);
            if slope.abs() < FLAT {
                return self.halved(x);
            }
            let next = t - (cubic(self.x1, self.x2, t) - x) / slope;
            if !(0.0..=1.0).contains(&next) {
                return self.halved(x);
            }
            t = next;
        }
        t
    }

    fn halved(self, x: f64) -> f64 {
        let (mut low, mut high) = (0.0, 1.0);
        for _ in 0..HALVINGS {
            let middle = (low + high) / 2.0;
            match cubic(self.x1, self.x2, middle) < x {
                true => low = middle,
                false => high = middle,
            }
        }
        (low + high) / 2.0
    }
}

fn cubic(first: f64, second: f64, t: f64) -> f64 {
    let rest = 1.0 - t;
    3.0 * rest * rest * t * first + 3.0 * rest * t * t * second + t * t * t
}

fn cubic_slope(first: f64, second: f64, t: f64) -> f64 {
    let rest = 1.0 - t;
    3.0 * rest * rest * first + 6.0 * rest * t * (second - first) + 3.0 * t * t * (1.0 - second)
}

#[derive(Clone, Copy)]
struct Tangent {
    along: (f64, f64),
    reach: f64,
}

impl Tangent {
    fn of(slope: f64, bounded: bool) -> Tangent {
        let along = match slope.is_infinite() {
            true => (0.0, 1.0),
            false => {
                let length = slope.hypot(1.0);
                (1.0 / length, slope / length)
            }
        };
        let reach = match bounded {
            true => (1.0 / along.0).min(1.0 / along.1),
            false => (1.0 / along.0).min(LONGEST),
        };
        Tangent { along, reach }
    }

    fn at(self, share: f64) -> (f64, f64) {
        let length = share * self.reach;
        (length * self.along.0, length * self.along.1)
    }
}

fn largest_error(bezier: CubicBezier, easing: Easing) -> f64 {
    (0..SAMPLES)
        .map(|sample| {
            let x = (sample as f64 + 0.5) / SAMPLES as f64;
            (bezier.at(x) - easing.at(x)).abs()
        })
        .fold(0.0, f64::max)
}

fn closest<const N: usize>(grid: usize, error: impl Fn([f64; N]) -> f64) -> [f64; N] {
    let cells = (grid + 1).pow(N as u32);
    let (mut best, mut least) = ([0.0; N], f64::INFINITY);
    for cell in 0..cells {
        let mut point = [0.0; N];
        let mut rest = cell;
        for coordinate in &mut point {
            *coordinate = (rest % (grid + 1)) as f64 / grid as f64;
            rest /= grid + 1;
        }
        let found = error(point);
        if found < least {
            (best, least) = (point, found);
        }
    }
    let moves: Vec<[f64; N]> = (0..3usize.pow(N as u32))
        .map(|code| {
            let mut step = [0.0; N];
            let mut rest = code;
            for coordinate in &mut step {
                *coordinate = (rest % 3) as f64 - 1.0;
                rest /= 3;
            }
            step
        })
        .filter(|step| step.iter().any(|coordinate| *coordinate != 0.0))
        .collect();
    let mut stride = 1.0 / grid as f64;
    while stride > FINEST {
        let better = moves.iter().find_map(|step| {
            let mut point = best;
            for (coordinate, along) in point.iter_mut().zip(step) {
                *coordinate = (*coordinate + along * stride).clamp(0.0, 1.0);
            }
            let found = error(point);
            (found < least).then_some((point, found))
        });
        match better {
            Some(found) => (best, least) = found,
            None => stride /= 2.0,
        }
    }
    best
}
