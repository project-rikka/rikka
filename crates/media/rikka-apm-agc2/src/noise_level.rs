use rikka_apm_audio::{FRAME, Frame, SAMPLE_RATE};

use crate::common::energy_to_dbfs;

const FRAMES_PER_SECOND: f32 = 100.0;
const UPDATE_PERIOD_FRAMES: i32 = 500;
const ATTACK: f32 = 0.5;

#[derive(Clone, Debug)]
pub(crate) struct NoiseFloor {
    min_noise_energy: f32,
    first_period: bool,
    preliminary_set: bool,
    preliminary_energy: f32,
    noise_energy: f32,
    counter: i32,
}

impl NoiseFloor {
    pub(crate) fn new() -> NoiseFloor {
        let min_noise_energy = SAMPLE_RATE as f32 * 2.0 * 2.0 / FRAMES_PER_SECOND;
        NoiseFloor {
            min_noise_energy,
            first_period: true,
            preliminary_set: false,
            preliminary_energy: min_noise_energy,
            noise_energy: min_noise_energy,
            counter: UPDATE_PERIOD_FRAMES,
        }
    }

    pub(crate) fn analyze(&mut self, frame: &Frame, channels: usize) -> f32 {
        let energy = frame_energy(frame, channels);
        if energy <= self.min_noise_energy {
            return energy_to_dbfs(self.noise_energy, FRAME);
        }
        if self.preliminary_set {
            self.preliminary_energy = self.preliminary_energy.min(energy);
        } else {
            self.preliminary_energy = energy;
            self.preliminary_set = true;
        }
        if self.counter == 0 {
            self.first_period = false;
            self.noise_energy = smooth(self.noise_energy, self.preliminary_energy);
            self.counter = UPDATE_PERIOD_FRAMES;
            self.preliminary_set = false;
        } else if self.first_period {
            self.noise_energy = self.preliminary_energy;
            self.counter -= 1;
        } else {
            self.noise_energy = self.noise_energy.min(self.preliminary_energy);
            self.counter -= 1;
        }
        energy_to_dbfs(self.noise_energy, FRAME)
    }
}

fn frame_energy(frame: &Frame, channels: usize) -> f32 {
    (0..channels)
        .map(|channel| {
            frame
                .full(channel)
                .iter()
                .fold(0.0f32, |sum, x| sum + x * x)
        })
        .fold(
            0.0f32,
            |energy, channel| if energy < channel { channel } else { energy },
        )
}

fn smooth(current: f32, new: f32) -> f32 {
    if current < new {
        return ATTACK * new + (1.0 - ATTACK) * current;
    }
    new
}
