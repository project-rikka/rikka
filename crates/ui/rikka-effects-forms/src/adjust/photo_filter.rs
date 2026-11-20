use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::adjust::photo_filter::{DENSITY, Filter, PhotoFilter};
use rikka_types::Color;
use rikka_ui::{Element, tr};

const FILTERS: [Filter; 20] = [
    Filter::Warming85,
    Filter::WarmingLba,
    Filter::Warming81,
    Filter::Cooling80,
    Filter::CoolingLbb,
    Filter::Cooling82,
    Filter::Red,
    Filter::Orange,
    Filter::Yellow,
    Filter::Green,
    Filter::Cyan,
    Filter::Blue,
    Filter::Violet,
    Filter::Magenta,
    Filter::Sepia,
    Filter::DeepRed,
    Filter::DeepBlue,
    Filter::DeepEmerald,
    Filter::DeepYellow,
    Filter::Underwater,
];

impl Form for PhotoFilter {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let effect = tuner.current();
        let mut rows = vec![tuner.segments(
            usize::from(effect.custom),
            vec![
                tr!("effects.photo_filter.filter"),
                tr!("effects.photo_filter.color"),
            ],
            |effect: &mut PhotoFilter, at: usize| effect.custom = at == 1,
        )];
        rows.push(match effect.custom {
            false => tuner.choice(
                tr!("effects.photo_filter.filter"),
                FILTERS.iter().position(|filter| *filter == effect.filter),
                FILTERS.map(Filter::name).to_vec(),
                |effect: &mut PhotoFilter, at: usize| {
                    effect.filter = FILTERS[at];
                    effect.color = FILTERS[at].color();
                },
            ),
            true => rows::labelled(
                tr!("effects.photo_filter.color"),
                tuner.swatch(effect.color, |effect: &mut PhotoFilter, color: Color| {
                    effect.color = color;
                }),
            ),
        });
        rows.extend([
            tuner.amount(DENSITY),
            tuner.toggle(
                tr!("effects.adjust.preserve_luminosity"),
                effect.preserve_luminosity,
                |effect: &mut PhotoFilter, on: bool| effect.preserve_luminosity = on,
            ),
        ]);
        rows::form_sheet(rows)
    }
}
