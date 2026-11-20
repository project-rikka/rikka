use crate::framing::FrameBands;

#[derive(Clone, Debug)]
pub(crate) struct BlockDelayBuffer {
    delay: usize,
    buffer: Vec<Vec<Vec<f32>>>,
    last_insert: usize,
}

impl BlockDelayBuffer {
    pub(crate) fn new(channels: usize, bands: usize, delay_samples: usize) -> BlockDelayBuffer {
        BlockDelayBuffer {
            delay: delay_samples,
            buffer: vec![vec![vec![0.0; delay_samples]; bands]; channels],
            last_insert: 0,
        }
    }

    pub(crate) fn delay_signal(&mut self, frame: &mut [FrameBands]) {
        if self.delay == 0 {
            return;
        }
        let start = self.last_insert;
        let mut i = start;
        for (buffer, frame) in self.buffer.iter_mut().zip(frame.iter_mut()) {
            for (buffer, samples) in buffer.iter_mut().zip(frame.iter_mut()) {
                i = start;
                for sample in samples.iter_mut() {
                    std::mem::swap(&mut buffer[i], sample);
                    i = if i < self.delay - 1 { i + 1 } else { 0 };
                }
            }
        }
        self.last_insert = i;
    }
}
