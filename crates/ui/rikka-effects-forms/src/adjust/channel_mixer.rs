use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::channel_mixer::{ChannelMixer, KEYS};
use rikka_ui::{Element, Styled, TextStyled, View, local_state, palette, text, tr};
use rikka_widgets::{combo, row, spacer};

#[derive(Clone, Copy, Debug, Default)]
struct FormState {
    output: usize,
}

const GRAY: usize = 3;

impl Form for ChannelMixer {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let tuner = tuner.clone();
        let palette = palette();
        local_state(FormState::default(), move |shown: FormState, show| {
            let effect = tuner.current();
            let (output, outputs, at) = match effect.monochrome {
                true => (GRAY, vec![tr!("effects.channel_mixer.gray")], 0),
                false => (
                    shown.output,
                    vec![
                        tr!("effects.adjust.red"),
                        tr!("effects.adjust.green"),
                        tr!("effects.adjust.blue"),
                    ],
                    shown.output,
                ),
            };
            let mix = effect.outputs()[output];
            let total = mix.red + mix.green + mix.blue;
            let [red, green, blue, constant] = KEYS[output];
            let monochrome = effect.monochrome;
            rows::form_sheet(vec![
                rows::labelled(
                    tr!("effects.channel_mixer.output"),
                    combo(Some(at))
                        .options(outputs)
                        .on_select(move |at: usize| {
                            if !monochrome {
                                show(FormState { output: at });
                            }
                        })
                        .fill_width(),
                ),
                tuner.amount_as(tr!("effects.adjust.red"), red),
                tuner.amount_as(tr!("effects.adjust.green"), green),
                tuner.amount_as(tr!("effects.adjust.blue"), blue),
                row! {
                    text(tr!("effects.channel_mixer.total")).caption().muted();
                    spacer().grow(1.0);
                    text(format!("{total:+.0}%"))
                        .caption()
                        .color(match total > 100.0 {
                            true => palette.warning,
                            false => palette.text,
                        });
                }
                .fill_width()
                .any(),
                tuner.amount_as(tr!("effects.channel_mixer.constant"), constant),
                tuner.toggle(
                    tr!("effects.channel_mixer.monochrome"),
                    monochrome,
                    |effect: &mut ChannelMixer, on: bool| effect.monochrome = on,
                ),
            ])
        })
    }
}
