use crate::editor::GradientEdit;
use crate::editor::GradientStrip;
use crate::form::Form;
use crate::rows::{self, FIELD};
use crate::tuner::Tuner;
use rikka_effects::Gradient;
use rikka_effects::adjust::gradient_map::{GradientMap, PRESETS, Preset};
use rikka_effects::{Scale, Unit};
use rikka_types::Color;
use rikka_ui::{Element, MaterialSymbol, Space, Styled, View, local_state, tr};
use rikka_widgets::{button, icon, row, spacer};

#[derive(Clone, Copy, Debug, Default)]
struct FormState {
    selected: usize,
}

fn edited(effect: &mut GradientMap, selected: usize, edit: GradientEdit) -> usize {
    let gradient = &mut effect.stops;
    let selected = match edit {
        GradientEdit::Select(index) => index,
        GradientEdit::Insert(at) => gradient.insert(at),
        GradientEdit::Drag(at) => gradient.moved(selected, at),
        GradientEdit::RemoveSelected => {
            gradient.remove(selected);
            selected
        }
        GradientEdit::Midpoint(index, midpoint) => {
            gradient.set_midpoint(index, midpoint);
            selected
        }
    };
    selected.min(gradient.stops().len() - 1)
}

const LOCATION: Scale = Scale::new(0.0..=100.0).unit(Unit::Percent);

const MIDPOINT: Scale = Scale::new(5.0..=95.0).unit(Unit::Percent);

impl Form for GradientMap {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let tuner = tuner.clone();
        local_state(FormState::default(), move |shown: FormState, show| {
            let effect = tuner.current();
            let count = effect.stops.stops().len();
            let selected = shown.selected.min(count - 1);
            let stop = effect.stops.stops()[selected];
            let last = selected + 1 == count;
            let presetting = show.clone();
            let editing = show.clone();
            let removing = show.clone();
            let dragging = show.clone();
            let (moving, moved) =
                tuner.staged(move |effect: &mut GradientMap, edit: GradientEdit| {
                    let selected = edited(effect, selected, edit);
                    editing(FormState { selected });
                });
            let mut rows = vec![
                tuner.preset(
                    tr!("effects.gradient_map.preset"),
                    effect.preset(),
                    PRESETS.map(Preset::name).to_vec(),
                    move |effect: &mut GradientMap, at: usize| {
                        effect.stops = PRESETS[at].gradient();
                        presetting(FormState { selected: 0 });
                    },
                ),
                GradientStrip::new(&effect.stops, selected, moving, moved).any(),
                row! {
                    tuner.swatch(stop.color, move |effect: &mut GradientMap, color: Color| {
                        effect.stops.set_color(selected, color);
                    });
                    spacer().grow(1.0);
                    button(icon(MaterialSymbol::DELETE))
                        .ghost()
                        .small()
                        .icon_only()
                        .enabled(Gradient::removable(effect.stops.stops()))
                        .on_press(tuner.tap(move |effect: &mut GradientMap| {
                            let selected = edited(effect, selected, GradientEdit::RemoveSelected);
                            removing(FormState { selected });
                        }));
                }
                .centered()
                .gap(Space::SNUG)
                .fill_width()
                .any(),
                rows::labelled(
                    tr!("effects.gradient_map.location"),
                    tuner
                        .number((stop.at * 100.0).round(), &LOCATION)
                        .on_commit(tuner.commit(move |effect: &mut GradientMap, value: f32| {
                            let selected =
                                edited(effect, selected, GradientEdit::Drag(value / 100.0));
                            dragging(FormState { selected });
                        }))
                        .width(FIELD),
                ),
            ];
            if !last {
                rows.push(rows::labelled(
                    tr!("effects.gradient_map.midpoint"),
                    tuner
                        .number((stop.midpoint * 100.0).round(), &MIDPOINT)
                        .on_commit(tuner.commit(move |effect: &mut GradientMap, value: f32| {
                            effect.stops.set_midpoint(selected, value / 100.0);
                        }))
                        .width(FIELD),
                ));
            }
            rows.extend([
                tuner.toggle(
                    tr!("effects.gradient_map.reverse"),
                    effect.reversed,
                    |effect: &mut GradientMap, on: bool| effect.reversed = on,
                ),
                rows::note(tr!("effects.gradient_map.hint")),
            ]);
            rows::form_sheet(rows)
        })
    }
}
