use super::levels::channel_names;
use crate::editor::CurveEdit;
use crate::editor::CurveGraph;
use crate::editor::bins;
use crate::form::Form;
use crate::rows::{self, FIELD};
use crate::tuner::Tuner;
use rikka_effects::Scale;
use rikka_effects::adjust::curves::{Curves, PRESETS, Preset};
use rikka_image_edit::HistogramChannel;
use rikka_ui::{Element, Styled, View, local_state, tr};
use rikka_widgets::combo;

#[derive(Clone, Copy, Debug, Default)]
struct FormState {
    channel: usize,
    selected: Option<usize>,
}

fn edited(
    curves: &mut Curves,
    channel: usize,
    selected: Option<usize>,
    edit: CurveEdit,
) -> Option<usize> {
    let curve = curves.curve(channel);
    match edit {
        CurveEdit::Select(index) => Some(index),
        CurveEdit::Insert(x, y) => curve.insert(x, y).or(selected),
        CurveEdit::Move(index, x, y) => {
            curve.moved(index, x, y);
            Some(index)
        }
        CurveEdit::Remove(index) => match curve.remove(index) {
            true => None,
            false => selected,
        },
    }
}

fn point(curves: &Curves, shown: FormState) -> Option<(f32, f32)> {
    curves.curves()[shown.channel]
        .points()
        .get(shown.selected?)
        .copied()
}

fn set_point(curves: &mut Curves, shown: FormState, input: Option<f32>, output: Option<f32>) {
    let (Some(index), Some((x, y))) = (shown.selected, point(curves, shown)) else {
        return;
    };
    let x = input.map_or(x, |input| input / 255.0);
    let y = output.map_or(y, |output| output / 255.0);
    curves.curve(shown.channel).moved(index, x, y);
}

const MEASURED: [HistogramChannel; 4] = [
    HistogramChannel::Luminosity,
    HistogramChannel::Red,
    HistogramChannel::Green,
    HistogramChannel::Blue,
];

const BINS: usize = 128;

const LEVEL: Scale = Scale::new(0.0..=255.0);

impl Form for Curves {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let tuner = tuner.clone();
        local_state(FormState::default(), move |shown: FormState, show| {
            let curves = tuner.current();
            let channel = shown.channel;
            let histogram = tuner
                .histogram()
                .map(|histogram| bins(histogram, MEASURED[channel], BINS));
            let drawn: [Vec<(f32, f32)>; 4] =
                std::array::from_fn(|at| curves.curves()[at].points().to_vec());
            let point = point(curves, shown);
            let (input, output) = point.map_or((0.0, 0.0), |(x, y)| (x * 255.0, y * 255.0));
            let choosing = show.clone();
            let editing = show.clone();
            let presetting = show.clone();
            let (drawing, drawn_curve) =
                tuner.staged(move |curves: &mut Curves, edit: CurveEdit| {
                    let selected = edited(curves, channel, shown.selected, edit);
                    editing(FormState { channel, selected });
                });
            rows::form_sheet(vec![
                tuner.preset(
                    tr!("effects.curves.preset"),
                    curves.preset(channel),
                    PRESETS.map(Preset::name).to_vec(),
                    move |curves: &mut Curves, at: usize| {
                        *curves.curve(channel) = PRESETS[at].curve();
                        presetting(FormState {
                            channel,
                            selected: None,
                        });
                    },
                ),
                rows::labelled(
                    tr!("effects.adjust.channel"),
                    combo(Some(channel))
                        .options(channel_names())
                        .on_select(move |at: usize| {
                            choosing(FormState {
                                channel: at,
                                selected: None,
                            });
                        })
                        .fill_width(),
                ),
                CurveGraph::new(channel, drawn, drawing, drawn_curve)
                    .selected(shown.selected)
                    .histogram(histogram)
                    .any(),
                rows::labelled(
                    tr!("effects.curves.input"),
                    tuner
                        .number(input.round(), &LEVEL)
                        .on_commit(tuner.commit(move |curves: &mut Curves, value: f32| {
                            set_point(curves, shown, Some(value), None);
                        }))
                        .enabled(point.is_some())
                        .width(FIELD),
                ),
                rows::labelled(
                    tr!("effects.curves.output"),
                    tuner
                        .number(output.round(), &LEVEL)
                        .on_commit(tuner.commit(move |curves: &mut Curves, value: f32| {
                            set_point(curves, shown, None, Some(value));
                        }))
                        .enabled(point.is_some())
                        .width(FIELD),
                ),
                rows::note(tr!("effects.curves.hint")),
            ])
        })
    }
}
