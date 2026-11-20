use crate::editor::Notch;
use crate::editor::Ramp;
use crate::editor::bins;
use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::threshold::{LEVEL, LEVEL_KNOB, Threshold};
use rikka_image_edit::HistogramChannel;
use rikka_types::Color;
use rikka_ui::{Element, View, palette};

const BINS: usize = 256;

const GREY: Color = Color::rgb(0.5, 0.5, 0.5);

impl Form for Threshold {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let histogram = tuner
            .histogram()
            .map(|histogram| bins(histogram, HistogramChannel::Luminosity, BINS))
            .unwrap_or_default();
        let (moving, moved) =
            tuner.staged(|threshold: &mut Threshold, (_, along): (usize, f32)| {
                threshold.level = LEVEL.clamp((along * 255.0).round());
            });
        rows::form_sheet(vec![
            Ramp::new(
                vec![Notch::new(tuner.current().level / 255.0, GREY)],
                moving,
                moved,
            )
            .histogram(Some(histogram), palette().text_muted)
            .any(),
            tuner.amount(LEVEL_KNOB),
        ])
    }
}
