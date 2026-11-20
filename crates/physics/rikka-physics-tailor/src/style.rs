use serde::{Deserialize, Serialize};

pub const CURVE_SAMPLES: usize = 16;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    points: Vec<[f32; 2]>,
}

impl Curve {
    #[must_use]
    pub fn flat(value: f32) -> Curve {
        Curve {
            points: vec![[0.0, value]],
        }
    }

    #[must_use]
    pub fn linear(root: f32, tip: f32) -> Curve {
        Curve {
            points: vec![[0.0, root], [1.0, tip]],
        }
    }

    #[must_use]
    pub fn from_points(points: impl IntoIterator<Item = [f32; 2]>) -> Curve {
        let mut points: Vec<[f32; 2]> = points
            .into_iter()
            .filter(|[depth, value]| depth.is_finite() && value.is_finite())
            .map(|[depth, value]| [depth.clamp(0.0, 1.0), value])
            .collect();
        points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        points.dedup_by(|later, earlier| later[0] == earlier[0]);
        if points.is_empty() {
            Curve::flat(0.0)
        } else {
            Curve { points }
        }
    }

    #[must_use]
    pub fn points(&self) -> &[[f32; 2]] {
        &self.points
    }

    #[must_use]
    pub fn sample(&self, depth: f32) -> f32 {
        let depth = depth.clamp(0.0, 1.0);
        let after = self.points.partition_point(|[at, _]| *at <= depth);
        match (
            after.checked_sub(1).map(|at| self.points[at]),
            self.points.get(after).copied(),
        ) {
            (Some([_, value]), None) | (None, Some([_, value])) => value,
            (Some([from, low]), Some([to, high])) => {
                let t = (depth - from) / (to - from);
                low + (high - low) * t
            }
            (None, None) => 0.0,
        }
    }

    #[must_use]
    pub fn scaled(&self, factor: f32) -> Curve {
        Curve {
            points: self
                .points
                .iter()
                .map(|[depth, value]| [*depth, value * factor])
                .collect(),
        }
    }

    #[must_use]
    pub fn table(&self) -> [f32; CURVE_SAMPLES] {
        std::array::from_fn(|index| self.sample(index as f32 / (CURVE_SAMPLES - 1) as f32))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Inertia {
    pub carried: f32,
    pub carried_turn: f32,
    pub depth_carried: f32,
    pub centrifugal: f32,
    pub speed_limit: f32,
    pub turn_limit: f32,
    pub smoothing: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flutter {
    pub amplitude: f32,
    pub frequency: f32,
    pub wavelength: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub gravity: f32,
    pub gravity_falloff: f32,
    pub damping: Curve,
    pub settling: f32,
    pub drag: f32,
    pub lift: f32,
    pub stretch: f32,
    pub compression: f32,
    pub bend: f32,
    pub buckling: f32,
    pub restore: Curve,
    pub limit: Curve,
    pub limit_stiffness: f32,
    pub pull: Curve,
    pub max_distance: Curve,
    pub backstop: Option<f32>,
    pub tether: f32,
    pub extent: f32,
    pub inertia: Inertia,
    pub max_speed: f32,
    pub attenuation: f32,
    pub friction: f32,
    pub thickness: f32,
    pub self_collision: bool,
    pub flutter: Flutter,
}
