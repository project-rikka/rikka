#[derive(Debug, Default)]
pub(crate) struct Ramp {
    current: Option<f32>,
}

impl Ramp {
    pub(crate) fn advance(&mut self, target: f32, frames: usize) -> (f32, f32) {
        let start = self.current.replace(target).unwrap_or(target);
        let step = if frames > 0 {
            (target - start) / frames as f32
        } else {
            0.0
        };
        (start, step)
    }

    pub(crate) fn apply(&mut self, buffer: &mut [f32], channels: usize, target: f32) {
        let (start, step) = self.advance(target, buffer.len() / channels);
        if start == 1.0 && step == 0.0 {
            return;
        }
        let mut gain = start;
        for frame in buffer.chunks_exact_mut(channels) {
            gain += step;
            for sample in frame {
                *sample *= gain;
            }
        }
    }
}
