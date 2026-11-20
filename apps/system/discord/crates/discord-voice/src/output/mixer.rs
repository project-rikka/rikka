use crate::output::limiter::Limiter;
use crate::output::stream::{CHUNK_SAMPLES, OUTPUT_CHANNELS};

#[derive(Debug)]
pub(crate) struct Mixer {
    mix: Vec<f32>,
    limiter: Limiter,
}

impl Mixer {
    pub(crate) fn new() -> Mixer {
        Mixer {
            mix: vec![0.0; CHUNK_SAMPLES],
            limiter: Limiter::new(),
        }
    }

    pub(crate) fn begin(&mut self) {
        self.mix.fill(0.0);
    }

    pub(crate) fn add(&mut self, audio: &[f32], gain: f32) {
        if gain == 0.0 {
            return;
        }
        for (mixed, sample) in self.mix.iter_mut().zip(audio) {
            *mixed += sample * gain;
        }
    }

    pub(crate) fn finish(&mut self, volume: f32) -> &[f32] {
        for sample in &mut self.mix {
            *sample *= volume;
        }
        self.limiter
            .process(&mut self.mix, usize::from(OUTPUT_CHANNELS));
        &self.mix
    }
}
