use std::f64::consts::PI;

use rikka_apm_audio::{BAND, BANDS, Frame};

struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 * (1.0 / 16_777_216.0) * 2.0 - 1.0
    }
}

const NOISE_AMPLITUDE: [f32; BANDS] = [800.0, 400.0, 200.0];
pub(crate) const BURSTS: [(usize, usize); 2] = [(260, 100), (440, 80)];

pub(crate) fn burst(frame: usize, sample: usize, channel: usize, band: usize) -> f64 {
    let Some(&(start, length)) = BURSTS
        .iter()
        .find(|&&(start, length)| (start..start + length).contains(&frame))
    else {
        return 0.0;
    };
    let position = ((frame - start) * BAND + sample) as f64 / (length * BAND) as f64;
    let rise = (PI * position).sin();
    let envelope = rise * rise;
    let time = (frame * BAND + sample) as f64 / 16_000.0;
    match band {
        0 => {
            let fundamental = 180.0 + 20.0 * channel as f64;
            let voice: f64 = (1..=8)
                .map(|k| {
                    4000.0 / f64::from(k) * (2.0 * PI * f64::from(k) * fundamental * time).sin()
                })
                .sum();
            envelope * voice
        }
        1 => envelope * 800.0 * (2.0 * PI * 3000.0 * time).sin(),
        _ => 0.0,
    }
}

pub(crate) struct Scene {
    noise: Vec<Lcg>,
    frame: usize,
    silent_frames: usize,
    noise_amplitude: [f32; BANDS],
    with_bursts: bool,
}

impl Scene {
    pub(crate) fn new(channels: usize) -> Scene {
        Scene {
            noise: (0..channels as u32)
                .map(|c| Lcg(0x1234 + 7919 * c))
                .collect(),
            frame: 0,
            silent_frames: 3,
            noise_amplitude: NOISE_AMPLITUDE,
            with_bursts: true,
        }
    }

    pub(crate) fn noise_only(amplitude: [f32; BANDS]) -> Scene {
        Scene {
            noise_amplitude: amplitude,
            with_bursts: false,
            silent_frames: 0,
            ..Scene::new(1)
        }
    }

    pub(crate) fn fill(&mut self, frame: &mut Frame) {
        let index = self.frame;
        for (channel, noise) in self.noise.iter_mut().enumerate() {
            for (band, samples) in frame.bands_mut(channel).iter_mut().enumerate() {
                for (sample, value) in samples.iter_mut().enumerate() {
                    let noise = noise.next() * self.noise_amplitude[band];
                    let voice = if self.with_bursts {
                        burst(index, sample, channel, band) as f32
                    } else {
                        0.0
                    };
                    *value = if index < self.silent_frames {
                        0.0
                    } else {
                        noise + voice
                    };
                }
            }
        }
        self.frame += 1;
    }
}
