#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remix {
    from: u16,
    to: u16,
}

impl Remix {
    #[must_use]
    pub fn new(from: u16, to: u16) -> Remix {
        Remix {
            from: from.max(1),
            to: to.max(1),
        }
    }

    #[must_use]
    pub fn from(&self) -> u16 {
        self.from
    }

    #[must_use]
    pub fn to(&self) -> u16 {
        self.to
    }

    #[must_use]
    pub fn mixes(&self) -> bool {
        self.to < self.from
    }

    pub fn apply(&self, input: &[f32], out: &mut Vec<f32>) {
        let from = usize::from(self.from);
        let to = usize::from(self.to);
        out.reserve(input.len() / from * to);
        for frame in input.chunks_exact(from) {
            for channel in 0..to {
                out.push(self.channel(frame, channel, from, to));
            }
        }
    }

    fn channel(&self, frame: &[f32], channel: usize, from: usize, to: usize) -> f32 {
        if from == 1 {
            return if channel < 2 { frame[0] } else { 0.0 };
        }
        if to >= from {
            return frame.get(channel).copied().unwrap_or(0.0);
        }
        let folded = frame.iter().skip(channel).step_by(to);
        let count = folded.clone().count().max(1);
        folded.sum::<f32>() / count as f32
    }
}
