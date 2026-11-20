use rikka_apm_aec3::EchoCanceller;
use rikka_apm_agc2::{AdaptiveDigital, GainController};
use rikka_apm_audio::{FRAME, Frame};
use rikka_apm_ns::{Level, NoiseSuppressor};
use rikka_simd::FlushDenormals;

use crate::high_pass::HighPass;
use crate::reference::Reference;

const MONO: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub high_pass: bool,
    pub echo_cancellation: bool,
    pub noise_suppression: Option<Level>,
    pub gain_control: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            high_pass: true,
            echo_cancellation: true,
            noise_suppression: Some(Level::High),
            gain_control: true,
        }
    }
}

pub struct Processor {
    config: Config,
    capture: Frame,
    render: Frame,
    reference: Reference,
    high_pass: HighPass,
    echo: Option<EchoCanceller>,
    noise: Option<NoiseSuppressor>,
    gain: Option<GainController>,
}

impl Processor {
    #[must_use]
    pub fn new(config: &Config) -> (Processor, Reference) {
        let reference = Reference::default();
        let mut processor = Processor {
            config: Config {
                high_pass: false,
                echo_cancellation: false,
                noise_suppression: None,
                gain_control: false,
            },
            capture: Frame::new(MONO),
            render: Frame::new(MONO),
            reference: reference.clone(),
            high_pass: HighPass::default(),
            echo: None,
            noise: None,
            gain: None,
        };
        processor.configure(config);
        (processor, reference)
    }

    pub fn configure(&mut self, config: &Config) {
        let before = std::mem::replace(&mut self.config, *config);
        if config.high_pass != before.high_pass {
            self.high_pass = HighPass::default();
        }
        if config.echo_cancellation != before.echo_cancellation {
            self.reference.clear();
            self.echo = config
                .echo_cancellation
                .then(|| EchoCanceller::new(&rikka_apm_aec3::Config::default(), MONO, MONO));
        }
        if config.noise_suppression != before.noise_suppression {
            self.noise = config
                .noise_suppression
                .map(|level| NoiseSuppressor::new(level, MONO));
        }
        if config.gain_control != before.gain_control {
            self.gain = config.gain_control.then(|| {
                let adaptive = rikka_apm_agc2::Config {
                    adaptive: AdaptiveDigital {
                        enabled: true,
                        ..AdaptiveDigital::default()
                    },
                    ..rikka_apm_agc2::Config::default()
                };
                GainController::new(&adaptive, MONO)
            });
        }
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn clean(&mut self, samples: &mut [f32; FRAME]) {
        let _flush = FlushDenormals::new();
        self.feed_render();
        self.capture.load(samples, MONO);
        if self.config.high_pass {
            self.high_pass.process(self.capture.full_mut(0));
        }
        if self.echo.is_none() && self.noise.is_none() {
            self.capture.store(samples, MONO);
            return;
        }
        if let Some(echo) = &mut self.echo {
            echo.analyze_capture(&self.capture);
        }
        self.capture.split();
        if let Some(noise) = &mut self.noise {
            noise.analyze(&self.capture);
        }
        if let Some(echo) = &mut self.echo {
            echo.process_capture(&mut self.capture, false);
        }
        if let Some(noise) = &mut self.noise {
            noise.process(&mut self.capture);
        }
        self.capture.merge();
        self.capture.store(samples, MONO);
    }

    pub fn level(&mut self, samples: &mut [f32; FRAME], speech_probability: Option<f32>) {
        let Some(gain) = &mut self.gain else {
            return;
        };
        let _flush = FlushDenormals::new();
        self.capture.load(samples, MONO);
        gain.process(&mut self.capture, speech_probability, false);
        self.capture.store(samples, MONO);
    }

    #[must_use]
    pub fn speech_probability(&self) -> Option<f32> {
        self.gain.as_ref()?.speech_probability()
    }

    fn feed_render(&mut self) {
        while let Some(rendered) = self.reference.take() {
            let Some(echo) = &mut self.echo else {
                continue;
            };
            self.render.load(&rendered, MONO);
            self.render.split();
            echo.render(&self.render);
        }
    }
}

impl std::fmt::Debug for Processor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Processor")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
