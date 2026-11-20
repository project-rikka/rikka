use std::ops::Range;

use crate::frame::{Frame, MAX_CHANNELS};
use crate::gains::{BlockGains, Fold};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pan {
    gains: BlockGains,
    fold: Fold,
    source: usize,
    output: usize,
}

impl Pan {
    #[must_use]
    pub fn new(pan: f32, source_channels: u16, output_channels: u16) -> Pan {
        let source = usize::from(source_channels).clamp(1, MAX_CHANNELS);
        let output = usize::from(output_channels).clamp(1, MAX_CHANNELS);
        Pan {
            gains: BlockGains::target(1.0, pan, source == 1),
            fold: Fold::select(source, output),
            source,
            output,
        }
    }

    pub fn mix(&self, source: &[f32], gain: Range<f32>, destination: &mut [f32]) {
        let frames = (source.len() / self.source).min(destination.len() / self.output);
        if frames == 0 {
            return;
        }
        let step = (gain.end - gain.start) / frames as f32;
        let mut scaled: Frame = [0.0; MAX_CHANNELS];
        let pairs = source
            .chunks_exact(self.source)
            .zip(destination.chunks_exact_mut(self.output));
        for (index, (from, into)) in pairs.take(frames).enumerate() {
            let amplitude = gain.start + step * (index + 1) as f32;
            for (slot, value) in scaled.iter_mut().zip(from) {
                *slot = value * amplitude;
            }
            self.gains.mix_frame(into, &scaled, self.source, self.fold);
        }
    }
}
