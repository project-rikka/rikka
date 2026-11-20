use desk_layout::{Content, Layout};
use shell_kit::Period;

use crate::widgets::photos;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Needs {
    pub(crate) weather: bool,
    pub(crate) player: bool,
    pub(crate) waveform: bool,
    pub(crate) timeline: bool,
    pub(crate) machine: bool,
    pub(crate) power: bool,
    pub(crate) devices: bool,
    pub(super) cadence: Period,
}

impl Needs {
    pub(super) const EVERYTHING: Needs = Needs {
        weather: true,
        player: true,
        waveform: true,
        timeline: true,
        machine: true,
        power: true,
        devices: true,
        cadence: Period::Second,
    };

    #[must_use]
    pub(super) fn for_layout(layout: &Layout) -> Needs {
        let mut needs = Needs {
            cadence: Period::Minute,
            ..Needs::default()
        };
        for widget in &layout.widgets {
            match &widget.content {
                Content::Clock(clock) if clock.seconds => {
                    needs.cadence = Period::Second;
                }
                Content::Weather(_) => needs.weather = true,
                Content::Media(media) => {
                    needs.player = true;
                    needs.waveform |= media.waveform;
                    needs.timeline = true;
                }
                Content::Monitor(_) => needs.machine = true,
                Content::Power(power) => {
                    needs.power = true;
                    needs.devices |= power.devices;
                }
                Content::Photos(photos) => {
                    needs.cadence = needs.cadence.min(photos::period(photos.interval));
                }
                Content::Clock(_) | Content::Calendar(_) | Content::Note(_) => {}
            }
        }
        needs
    }
}
