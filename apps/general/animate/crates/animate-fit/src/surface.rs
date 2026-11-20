use rikka_model_core::glam::Vec3;

const MOST_STEPS: usize = 24;

pub(crate) struct Sample {
    pub(crate) point: Vec3,
    pub(crate) weights: [f32; 3],
}

impl Sample {
    pub(crate) fn nearest(&self) -> usize {
        (0..3)
            .max_by(|one, other| self.weights[*one].total_cmp(&self.weights[*other]))
            .unwrap_or(0)
    }

    pub(crate) fn blend(&self, values: [f32; 3]) -> f32 {
        values
            .iter()
            .zip(self.weights)
            .map(|(value, weight)| value * weight)
            .sum()
    }
}

pub(crate) fn samples(corners: [Vec3; 3], spacing: f32) -> impl Iterator<Item = Sample> {
    let longest = (0..3)
        .map(|side| corners[side].distance(corners[(side + 1) % 3]))
        .fold(0.0, f32::max);
    let steps = ((longest / spacing).ceil() as usize).clamp(1, MOST_STEPS);
    (0..=steps).flat_map(move |one| {
        (0..=steps - one).map(move |two| {
            let weights = [
                one as f32 / steps as f32,
                two as f32 / steps as f32,
                (steps - one - two) as f32 / steps as f32,
            ];
            Sample {
                point: corners[0] * weights[0] + corners[1] * weights[1] + corners[2] * weights[2],
                weights,
            }
        })
    })
}
