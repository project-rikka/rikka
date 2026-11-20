use glam::Vec3;

const WEIGHTLESS: f32 = 1e-4;
const NEARBY: f32 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Throw {
    pub velocity: Vec3,
    pub seconds: f32,
}

impl Throw {
    pub const STILL: Self = Self {
        velocity: Vec3::ZERO,
        seconds: 0.0,
    };

    #[must_use]
    pub fn ballistic(from: Vec3, to: Vec3, gravity: Vec3, arc: f32) -> Option<Self> {
        let pull = gravity.length();
        if pull <= WEIGHTLESS {
            return None;
        }
        let up = -gravity / pull;
        let offset = to - from;
        let rise = offset.dot(up);
        let across = offset - up * rise;
        let apex = rise.max(0.0) + arc.max(0.0);
        let climb = (2.0 * pull * apex).sqrt();
        let seconds = climb / pull + (2.0 * (apex - rise).max(0.0) / pull).sqrt();
        if seconds <= NEARBY {
            return (across.length() <= NEARBY).then_some(Self::STILL);
        }
        Some(Self {
            velocity: across / seconds + up * climb,
            seconds,
        })
    }

    #[must_use]
    pub fn direct(from: Vec3, to: Vec3, gravity: Vec3, speed: f32) -> Self {
        let pull = gravity.length();
        let up = if pull > WEIGHTLESS {
            -gravity / pull
        } else {
            Vec3::Y
        };
        let offset = to - from;
        let distance = offset.length();
        let heading = if distance > NEARBY {
            offset / distance
        } else {
            -up
        };
        let speed = speed.max(0.0);
        let velocity = heading * speed;
        let rise = offset.dot(up);
        let sideways = (offset - up * rise).length() > NEARBY;
        let seconds = match (speed > NEARBY, pull > WEIGHTLESS) {
            (true, false) => distance / speed,
            (true, true) if sideways => distance / speed,
            (_, true) => landing(velocity.dot(up), rise, pull),
            (false, false) => 0.0,
        };
        Self { velocity, seconds }
    }

    #[must_use]
    pub fn at(&self, from: Vec3, gravity: Vec3, seconds: f32) -> Vec3 {
        from + self.velocity * seconds + gravity * (0.5 * seconds * seconds)
    }

    #[must_use]
    pub fn path(&self, from: Vec3, gravity: Vec3, points: usize) -> Vec<Vec3> {
        let points = points.max(2);
        let last = (points - 1) as f32;
        (0..points)
            .map(|at| self.at(from, gravity, self.seconds * at as f32 / last))
            .collect()
    }
}

fn landing(climb: f32, rise: f32, pull: f32) -> f32 {
    let reach = climb * climb - 2.0 * pull * rise;
    if reach >= 0.0 {
        (climb + reach.sqrt()) / pull
    } else {
        climb / pull
    }
}
