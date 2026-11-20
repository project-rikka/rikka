use crate::common::{SPECTRUM_LEN, Spectrum};

#[derive(Clone, Debug)]
pub(crate) struct MovingAverage {
    scaling: f32,
    memory: Vec<Spectrum>,
    index: usize,
}

impl MovingAverage {
    pub(crate) fn new(len: usize) -> MovingAverage {
        MovingAverage {
            scaling: 1.0 / len as f32,
            memory: vec![[0.0; SPECTRUM_LEN]; len - 1],
            index: 0,
        }
    }

    pub(crate) fn average(&mut self, input: &Spectrum, output: &mut Spectrum) {
        *output = *input;
        for memory in &self.memory {
            for (out, value) in output.iter_mut().zip(memory) {
                *out += value;
            }
        }
        for out in output.iter_mut() {
            *out *= self.scaling;
        }
        if !self.memory.is_empty() {
            self.memory[self.index] = *input;
            self.index = (self.index + 1) % self.memory.len();
        }
    }
}
