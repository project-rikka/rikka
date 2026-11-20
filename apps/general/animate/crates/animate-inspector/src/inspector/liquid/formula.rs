use std::sync::Arc;

use animate_fluid::{Emitter, EmitterValue, Preset, Property, ValueKind};
use animate_ui::{LABEL_WIDTH, framed, glyph, heading};
use rikka_app::prelude::*;

use crate::inspector::liquid::row::picker;
use crate::{Desk, Reach, wording};

pub(super) fn card(desk: Desk<'_>, emitter: &Arc<Emitter>) -> Element {
    let preset = emitter.liquid.preset();
    let chosen = Preset::ALL
        .iter()
        .position(|candidate| *candidate == preset);
    let mut parts = vec![
        heading(tr!("animate.liquid.liquid")),
        picker(chosen, Preset::ALL.map(wording::preset).to_vec())
            .on_select(desk.act_with(move |studio, index: usize| {
                if let Some(preset) = Preset::ALL.get(index) {
                    studio.choose_preset(*preset);
                }
            }))
            .fill_width()
            .any(),
    ];
    parts.extend(
        Property::ALL
            .iter()
            .map(|property| row(desk, emitter, *property)),
    );
    framed(parts)
}

fn row(desk: Desk<'_>, emitter: &Emitter, property: Property) -> Element {
    let overridden = emitter.liquid.overrides_property(property);
    let name = text(wording::property(property))
        .caption()
        .no_wrap()
        .width(LABEL_WIDTH);
    let name = match overridden {
        true => name.color(palette().accent).bold(),
        false => name,
    };
    let value = desk.mixed(emitter, property);
    let control = match property.kind() {
        ValueKind::Color => color(desk.reach(), property, value.three().unwrap_or([1.0; 3])),
        ValueKind::Amount { min, max } | ValueKind::Angle { min, max } => amount(
            desk.reach(),
            property,
            value.number().unwrap_or(min),
            min,
            max,
        ),
        ValueKind::Position => spacer().any(),
    };
    let reset = glyph(MaterialSymbol::RESTART_ALT)
        .ghost()
        .enabled(overridden)
        .on_press(desk.act(move |studio| studio.reset_property(property)))
        .tooltip(tr!("animate.tip.reset"));
    row_of([name.any(), container(control).grow(1.0).any(), reset])
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any()
}

fn amount(reach: Reach, property: Property, value: f32, min: f32, max: f32) -> Element {
    slider(value.clamp(min, max))
        .range(min..=max)
        .control_size(ControlSize::Small)
        .on_change(reach.act_with(move |studio, value: f32| {
            studio.draft_property(property, EmitterValue::Number(value));
        }))
        .on_commit(reach.act_with(move |studio, value: f32| {
            studio.apply_property(property, EmitterValue::Number(value));
        }))
        .fill_width()
        .any()
}

fn color(reach: Reach, property: Property, rgb: [f32; 3]) -> Element {
    let three = |color: Color| EmitterValue::Three([color.r, color.g, color.b]);
    row_of([
        color_well(Color::rgb(rgb[0], rgb[1], rgb[2]))
            .control_size(ControlSize::Small)
            .on_change(reach.act_with(move |studio, color: Color| {
                studio.draft_property(property, three(color));
            }))
            .on_commit(reach.act_with(move |studio, color: Color| {
                studio.apply_property(property, three(color));
            }))
            .any(),
        spacer().grow(1.0).any(),
    ])
    .align_items(Align::Center)
    .fill_width()
    .any()
}
