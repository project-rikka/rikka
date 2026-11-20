use std::time::Duration;

use crate::channel::Channel;

const CRITICAL_BAND: f32 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub damping_ratio: f32,
    pub stiffness: f32,
}

impl Spring {
    pub const FAST_SPATIAL: Self = Self::new(0.6, 800.0);
    pub const DEFAULT_SPATIAL: Self = Self::new(0.8, 380.0);
    pub const SLOW_SPATIAL: Self = Self::new(0.8, 200.0);
    pub const FAST_EFFECTS: Self = Self::new(1.0, 3800.0);
    pub const DEFAULT_EFFECTS: Self = Self::new(1.0, 1600.0);
    pub const SLOW_EFFECTS: Self = Self::new(1.0, 800.0);

    pub const fn new(damping_ratio: f32, stiffness: f32) -> Self {
        Self {
            damping_ratio,
            stiffness,
        }
    }

    pub const fn overshoots(self) -> bool {
        self.damping_ratio < 1.0
    }

    #[must_use]
    pub fn launch_speed(self, distance: f32) -> f32 {
        distance * self.stiffness.max(0.0).sqrt()
    }

    #[inline]
    pub fn step(self, channel: &mut Channel, target: f32, elapsed: Duration) {
        let t = elapsed.as_secs_f32();
        if t <= 0.0 {
            return;
        }
        let (gap, velocity) = self.sample(channel.gap_to(target), channel.velocity, t);
        channel.place(target, gap, velocity);
    }

    fn sample(self, x0: f32, v0: f32, t: f32) -> (f32, f32) {
        let omega = self.stiffness.max(f32::EPSILON).sqrt();
        let zeta = self.damping_ratio.max(0.0);
        if zeta < 1.0 - CRITICAL_BAND {
            let omega_d = omega * (1.0 - zeta * zeta).sqrt();
            let b = (v0 + zeta * omega * x0) / omega_d;
            let decay = (-zeta * omega * t).exp();
            let (sin, cos) = (omega_d * t).sin_cos();
            let x = decay * (x0 * cos + b * sin);
            let v = -zeta * omega * x + decay * omega_d * (b * cos - x0 * sin);
            (x, v)
        } else if zeta > 1.0 + CRITICAL_BAND {
            let spread = (zeta * zeta - 1.0).sqrt();
            let r1 = -omega * (zeta - spread);
            let r2 = -omega * (zeta + spread);
            let c2 = (v0 - r1 * x0) / (r2 - r1);
            let c1 = x0 - c2;
            let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
            (c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2)
        } else {
            let b = v0 + omega * x0;
            let decay = (-omega * t).exp();
            let x = (x0 + b * t) * decay;
            (x, (b - omega * (x0 + b * t)) * decay)
        }
    }
}
