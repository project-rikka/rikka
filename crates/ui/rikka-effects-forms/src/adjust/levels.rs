use crate::editor::Notch;
use crate::editor::Ramp;
use crate::editor::bins;
use crate::form::Form;
use crate::rows::{self, FIELD};
use crate::tuner::Tuner;
use rikka_effects::adjust::levels::{KEYS, Levels, MEASURED};
use rikka_types::Color;
use rikka_ui::Label;
use rikka_ui::{Element, Space, Styled, TextStyled, View, local_state, palette, text, tr};
use rikka_widgets::{button, row, spacer};

#[derive(Clone, Copy, Debug, Default)]
struct FormState {
    channel: usize,
}

const BINS: usize = 256;

const GREY: Color = Color::rgb(0.5, 0.5, 0.5);

pub(crate) fn channel_names() -> Vec<Label> {
    vec![
        tr!("effects.adjust.rgb"),
        tr!("effects.adjust.red"),
        tr!("effects.adjust.green"),
        tr!("effects.adjust.blue"),
    ]
}

impl Form for Levels {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let tuner = tuner.clone();
        let tint = palette().text_muted;
        local_state(FormState::default(), move |shown: FormState, show| {
            let channel = shown.channel;
            let setting = tuner.current().settings()[channel];
            let [black, gamma, white, out_black, out_white] = KEYS[channel];
            let histogram = tuner
                .histogram()
                .map(|histogram| bins(histogram, MEASURED[channel], BINS));
            let measured = tuner.histogram().cloned();
            let showing = show.clone();
            let (moving_input, moved_input) =
                tuner.staged(move |levels: &mut Levels, (notch, along): (usize, f32)| {
                    levels.moved_input(channel, notch, along);
                });
            let (moving_output, moved_output) =
                tuner.staged(move |levels: &mut Levels, (notch, along): (usize, f32)| {
                    levels.moved_output(channel, notch, along);
                });
            rows::form_sheet(vec![
                rows::labelled(
                    tr!("effects.adjust.channel"),
                    rikka_widgets::combo(Some(channel))
                        .options(channel_names())
                        .on_select(move |at: usize| showing(FormState { channel: at }))
                        .fill_width(),
                ),
                row! {
                    text(tr!("effects.adjust.input_levels")).bold().grow(1.0).min_width(0.0);
                    button(text(tr!("effects.adjust.auto")))
                        .small()
                        .enabled(measured.is_some())
                        .on_press(tuner.tap(move |levels: &mut Levels| {
                            if let Some(histogram) = &measured {
                                levels.auto(histogram);
                            }
                        }));
                }
                .centered()
                .gap(Space::SNUG)
                .fill_width()
                .any(),
                Ramp::new(
                    vec![
                        Notch::new(setting.black / 255.0, Color::BLACK),
                        Notch::new(setting.midtone() / 255.0, GREY),
                        Notch::new(setting.white / 255.0, Color::WHITE),
                    ],
                    moving_input,
                    moved_input,
                )
                .histogram(Some(histogram.unwrap_or_default()), tint)
                .any(),
                row! {
                    tuner.field(black).width(FIELD);
                    tuner.adornment(black);
                    spacer().grow(1.0);
                    tuner.field(gamma).width(FIELD);
                    tuner.adornment(gamma);
                    spacer().grow(1.0);
                    tuner.field(white).width(FIELD);
                    tuner.adornment(white);
                }
                .centered()
                .fill_width()
                .any(),
                rows::heading(tr!("effects.adjust.output_levels")),
                Ramp::new(
                    vec![
                        Notch::new(setting.out_black / 255.0, Color::BLACK),
                        Notch::new(setting.out_white / 255.0, Color::WHITE),
                    ],
                    moving_output,
                    moved_output,
                )
                .any(),
                row! {
                    tuner.field(out_black).width(FIELD);
                    tuner.adornment(out_black);
                    spacer().grow(1.0);
                    tuner.field(out_white).width(FIELD);
                    tuner.adornment(out_white);
                }
                .centered()
                .fill_width()
                .any(),
            ])
        })
    }
}
