use std::time::Duration;

use rikka_apm::{Config, NoiseLevel, Processor, Reference};

use crate::input::gate::Gate;
use crate::input::suppress::Suppress;
use crate::krisp::Krisp;
use crate::level::rms_db;
use crate::settings::{InputMode, VoiceSettings};

pub(crate) const STEP: Duration = Duration::from_millis(10);
pub(crate) const STEP_SAMPLES: usize = rikka_apm::FRAME;
const STANDARD_LEVEL: NoiseLevel = NoiseLevel::High;

#[derive(Debug)]
pub(crate) struct Chain {
    volume: f32,
    mode: InputMode,
    echo_cancellation: bool,
    automatic_gain: bool,
    processor: Processor,
    suppress: Suppress,
    gate: Gate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Processed {
    pub(crate) level_db: f32,
    pub(crate) open: bool,
}

impl Chain {
    pub(crate) fn new(settings: &VoiceSettings, krisp: Krisp) -> (Chain, Reference) {
        let mut suppress = Suppress::new(krisp, settings.noise_suppression);
        let config = config(
            settings.echo_cancellation,
            settings.automatic_gain,
            suppress.needs_standard(),
        );
        let (processor, reference) = Processor::new(&config);
        let chain = Chain {
            volume: settings.input_gain(),
            mode: settings.mode,
            echo_cancellation: settings.echo_cancellation,
            automatic_gain: settings.automatic_gain,
            processor,
            suppress,
            gate: Gate::new(settings.mode),
        };
        (chain, reference)
    }

    pub(crate) fn configure(&mut self, settings: &VoiceSettings) {
        self.volume = settings.input_gain();
        self.echo_cancellation = settings.echo_cancellation;
        self.automatic_gain = settings.automatic_gain;
        self.suppress.set_mode(settings.noise_suppression);
        if settings.mode != self.mode {
            self.mode = settings.mode;
            self.gate = Gate::new(settings.mode);
        }
    }

    pub(crate) fn process(&mut self, frame: &mut [f32; STEP_SAMPLES], pressed: bool) -> Processed {
        for sample in frame.iter_mut() {
            *sample *= self.volume;
        }
        let apm_config = config(
            self.echo_cancellation,
            self.automatic_gain,
            self.suppress.needs_standard(),
        );
        if apm_config != *self.processor.config() {
            self.processor.configure(&apm_config);
        }
        self.processor.clean(frame);
        let krisp_probability = self.suppress.clean(frame);
        self.processor.level(frame, krisp_probability);
        let probability = krisp_probability.or_else(|| self.processor.speech_probability());
        let level_db = rms_db(frame);
        let open = self.gate.update(level_db, probability, pressed, STEP);
        Processed { level_db, open }
    }
}

fn config(echo_cancellation: bool, automatic_gain: bool, standard: bool) -> Config {
    Config {
        high_pass: true,
        echo_cancellation,
        noise_suppression: standard.then_some(STANDARD_LEVEL),
        gain_control: automatic_gain,
    }
}
