const NEWTON_STEPS: usize = 8;

const BISECTION_STEPS: usize = 32;

const EPSILON: f32 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    Emphasized,
    EmphasizedDecel,
    EmphasizedAccel,
    Standard,
    StandardDecel,
    StandardAccel,
    Linear,
}

impl Easing {
    pub const fn control_points(self) -> [f32; 4] {
        match self {
            Self::Emphasized | Self::Standard => [0.2, 0.0, 0.0, 1.0],
            Self::EmphasizedDecel => [0.05, 0.7, 0.1, 1.0],
            Self::EmphasizedAccel => [0.3, 0.0, 0.8, 0.15],
            Self::StandardDecel => [0.0, 0.0, 0.0, 1.0],
            Self::StandardAccel => [0.3, 0.0, 1.0, 1.0],
            Self::Linear => [0.0, 0.0, 1.0, 1.0],
        }
    }

    #[inline]
    pub fn transform(self, fraction: f32) -> f32 {
        let x = fraction.clamp(0.0, 1.0);
        if self == Self::Linear || x == 0.0 || x == 1.0 {
            return x;
        }
        let [x1, y1, x2, y2] = self.control_points();
        bezier(y1, y2, solve(x1, x2, x))
    }
}

#[inline]
fn bezier(p1: f32, p2: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
}

#[inline]
fn slope(p1: f32, p2: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * p1 + 6.0 * u * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

fn solve(x1: f32, x2: f32, x: f32) -> f32 {
    let mut t = x;
    for _ in 0..NEWTON_STEPS {
        let error = bezier(x1, x2, t) - x;
        if error.abs() < EPSILON {
            return t;
        }
        let derivative = slope(x1, x2, t);
        if derivative.abs() < EPSILON {
            break;
        }
        t = (t - error / derivative).clamp(0.0, 1.0);
    }
    let (mut low, mut high) = (0.0, 1.0);
    t = x;
    for _ in 0..BISECTION_STEPS {
        let found = bezier(x1, x2, t);
        if (found - x).abs() < EPSILON {
            break;
        }
        if found < x {
            low = t;
        } else {
            high = t;
        }
        t = (low + high) / 2.0;
    }
    t
}
