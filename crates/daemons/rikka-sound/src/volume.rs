fn cubed(level: f32) -> f32 {
    level * level * level
}

fn uncubed(amplitude: f32) -> f32 {
    amplitude.cbrt()
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Volume {
    channels: Vec<f32>,
    muted: bool,
}

impl Volume {
    pub fn new(channels: Vec<f32>, muted: bool) -> Self {
        Self { channels, muted }
    }

    pub fn channels(&self) -> &[f32] {
        &self.channels
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    pub fn muted_to(&self, muted: bool) -> Self {
        Self {
            channels: self.channels.clone(),
            muted,
        }
    }

    pub fn amplitude(&self) -> f32 {
        self.channels.iter().copied().fold(0.0, f32::max)
    }

    pub fn level(&self) -> f32 {
        uncubed(self.amplitude())
    }

    pub fn at_level(&self, level: f32) -> Self {
        let target = cubed(level.max(0.0));
        let current = self.amplitude();
        let channels = if current > 0.0 {
            self.channels
                .iter()
                .map(|channel| channel / current * target)
                .collect()
        } else {
            vec![target; self.channels.len().max(1)]
        };
        Self {
            channels,
            muted: self.muted,
        }
    }

    pub fn balance(&self) -> f32 {
        let Some([left, right]) = self.stereo() else {
            return 0.0;
        };
        let loudest = left.max(right);
        if loudest <= 0.0 {
            return 0.0;
        }
        (right - left) / loudest
    }

    pub fn at_balance(&self, balance: f32) -> Self {
        let Some([left, right]) = self.stereo() else {
            return self.clone();
        };
        let loudest = left.max(right);
        let balance = balance.clamp(-1.0, 1.0);
        Self {
            channels: vec![
                loudest * (1.0 - balance).min(1.0),
                loudest * (1.0 + balance).min(1.0),
            ],
            muted: self.muted,
        }
    }

    pub fn stereo(&self) -> Option<[f32; 2]> {
        match self.channels.as_slice() {
            [left, right] => Some([*left, *right]),
            _ => None,
        }
    }

    pub fn decibels(&self) -> Option<f32> {
        let amplitude = self.amplitude();
        (amplitude > 0.0).then(|| 20.0 * amplitude.log10())
    }
}
