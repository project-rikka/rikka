use cut_filmstrip::Mode;
use cut_waveform::Scale;
use rikka_settings::Schema;

use super::{Preferences, choice, name_of, named};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Display {
    pub thumbnails: Mode,
    pub waveform: Scale,
}

const THUMBNAILS: &str = "timeline-thumbnails";
const WAVEFORM: &str = "timeline-waveform";

const MODES: [(Mode, &str); 4] = [
    (Mode::Continuous, "continuous"),
    (Mode::Head, "head"),
    (Mode::HeadAndTail, "head-and-tail"),
    (Mode::Off, "off"),
];
const SCALES: [(Scale, &str); 2] = [
    (Scale::Logarithmic, "logarithmic"),
    (Scale::Linear, "linear"),
];

impl Preferences {
    #[must_use]
    pub fn display(&self) -> Display {
        let choice = |key: &str| self.0.get::<String>(key);
        Display {
            thumbnails: named(&MODES, &choice(THUMBNAILS)).unwrap_or_default(),
            waveform: named(&SCALES, &choice(WAVEFORM)).unwrap_or_default(),
        }
    }

    pub fn keep_display(&mut self, display: Display) {
        self.0.set(THUMBNAILS, name_of(&MODES, display.thumbnails));
        self.0.set(WAVEFORM, name_of(&SCALES, display.waveform));
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    let display = Display::default();
    schema
        .key(
            choice(THUMBNAILS, &MODES, display.thumbnails)
                .summary("How video clips on the timeline show their frames"),
        )
        .key(
            choice(WAVEFORM, &SCALES, display.waveform)
                .summary("Whether timeline waveforms are drawn in decibels or linearly"),
        )
}
