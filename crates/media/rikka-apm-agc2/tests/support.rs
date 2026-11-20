use rikka_apm_agc2::{AdaptiveDigital, Config, GainController};
use rikka_apm_audio::{FRAME, Frame};

#[derive(Clone, Debug)]
pub(crate) struct Synth {
    seed: u32,
    level: f32,
    noise_level: f32,
    period: i64,
    n: i64,
    next_pulse: i64,
    y1: f32,
    y2: f32,
    z1: f32,
    z2: f32,
}

impl Synth {
    pub(crate) fn new(seed: u32, level: f32, noise_level: f32, period: i64) -> Synth {
        Synth {
            seed,
            level,
            noise_level,
            period,
            n: 0,
            next_pulse: 0,
            y1: 0.0,
            y2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn draw(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / 16_777_216.0 * 2.0 - 1.0
    }

    pub(crate) fn next(&mut self) -> f32 {
        let aspiration = self.draw();
        let floor = self.draw();
        let mut excitation = 0.0f32;
        if self.n == self.next_pulse {
            excitation = 1.0;
            self.next_pulse += self.period + ((self.n / 4800) * 37) % 97;
        }
        excitation += 0.02 * aspiration;
        let a = excitation + 1.93187 * self.y1 - 0.9409 * self.y2;
        self.y2 = self.y1;
        self.y1 = a;
        let b = a + 1.89636 * self.z1 - 0.9216 * self.z2;
        self.z2 = self.z1;
        self.z1 = b;
        let k = self.n % 24_000;
        let envelope = if k < 18_000 {
            1.0f32.min((k as f32 / 2400.0).min((18_000 - k) as f32 / 2400.0))
        } else {
            0.0
        };
        self.n += 1;
        self.level * envelope * b + self.noise_level * floor
    }

    pub(crate) fn fill(&mut self, samples: &mut [f32; FRAME]) {
        for sample in samples {
            *sample = self.next();
        }
    }
}

pub(crate) fn adaptive(max_gain_db: f32) -> Config {
    Config {
        fixed_gain_db: 0.0,
        adaptive: AdaptiveDigital {
            enabled: true,
            max_gain_db,
            ..AdaptiveDigital::default()
        },
    }
}

pub(crate) fn run(
    controller: &mut GainController,
    synths: &mut [Synth],
    frames: usize,
    probability: impl Fn(usize) -> Option<f32>,
    mut inspect: impl FnMut(usize, &Frame, &GainController),
) {
    let mut frame = Frame::new(synths.len());
    for index in 0..frames {
        for (channel, synth) in synths.iter_mut().enumerate() {
            synth.fill(frame.full_mut(channel));
        }
        controller.process(&mut frame, probability(index), false);
        inspect(index, &frame, controller);
    }
}

pub(crate) fn rms_dbfs(samples: &[f32]) -> f32 {
    let energy: f64 = samples.iter().map(|&x| f64::from(x) * f64::from(x)).sum();
    (10.0 * (energy / samples.len() as f64).log10() - 20.0 * 32_768f64.log10()) as f32
}
