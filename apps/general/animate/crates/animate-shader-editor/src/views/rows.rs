use animate_look::Target;
use animate_shader::Merge;
use animate_ui::{glyph, labelled};
use rikka_app::prelude::*;
use rikka_shader_graph::{Constant, SocketValue};
use rikka_types::srgb;

use crate::canvas_data::shown_value;
use crate::sockets::Field;
use crate::traits::{KeyState, Reach, Workshop};

const FIELD: f32 = 58.0;
const CHANNELS: [&str; 4] = ["X", "Y", "Z", "W"];

pub(crate) struct ValueRow {
    pub(crate) key: Target,
    pub(crate) label: Label,
    pub(crate) field: Field,
    pub(crate) value: Option<SocketValue>,
    pub(crate) set: bool,
    pub(crate) document: Option<TextDocument>,
}

fn merge_of(key: &Target) -> Merge {
    Merge(rikka_ui::hash_of(key))
}

fn constant(value: &Option<SocketValue>, width: usize) -> Vec<f32> {
    let held = value
        .as_ref()
        .and_then(SocketValue::number)
        .unwrap_or(Constant::scalar(0.0));
    held.resized(width).values().to_vec()
}

fn draft(studio: &mut dyn Workshop, key: &Target, values: &[f32]) {
    if let Some(constant) = Constant::from_slice(values) {
        studio.set_value(key.clone(), Some(constant.into()), Some(merge_of(key)));
    }
}

fn commit(studio: &mut dyn Workshop, key: &Target, values: &[f32]) {
    draft(studio, key, values);
    studio.seal_look();
}

pub(crate) fn number_fields(
    reach: Reach,
    values: &[f32],
    step: f32,
    set: impl Fn(&mut dyn Workshop, usize, f32) + Clone + 'static,
) -> Element {
    row_of(values.iter().enumerate().map(|(index, value)| {
        let set = set.clone();
        number_field(*value)
            .step(step)
            .decimals(3)
            .control_size(ControlSize::Small)
            .on_commit(reach.act_with(move |studio, value: f32| set(studio, index, value)))
            .width(FIELD)
            .any()
    }))
    .gap(Space::TIGHT)
    .any()
}

fn number(reach: Reach, key: Target, value: f32, field: &Field) -> Element {
    let Field::Number {
        range,
        hard,
        integer,
        angle,
    } = *field
    else {
        return text("").any();
    };
    let scale = if angle { 1f32.to_radians() } else { 1.0 };
    let shown = value / scale;
    match range {
        Some((low, high)) => {
            let (low, high) = (low / scale, high / scale);
            let (low, high) = match hard {
                true => (low, high),
                false => (low.min(shown), high.max(shown)),
            };
            let (change, release) = (key.clone(), key);
            let bar = slider(shown.clamp(low, high))
                .range(low..=high)
                .control_size(ControlSize::Small)
                .on_change(reach.act_with(move |studio, value: f32| {
                    draft(studio, &change, &[value * scale]);
                }))
                .on_commit(reach.act_with(move |studio, value: f32| {
                    commit(studio, &release, &[value * scale]);
                }));
            match integer {
                true => bar.step(1.0).fill_width().any(),
                false => bar.fill_width().any(),
            }
        }
        None => number_field(shown)
            .step(if integer { 1.0 } else { 0.01 })
            .decimals(if integer { 0 } else { 3 })
            .control_size(ControlSize::Small)
            .on_commit(reach.act_with(move |studio, value: f32| {
                let value = match integer {
                    true => value.round(),
                    false => value,
                };
                commit(studio, &key, &[value * scale]);
            }))
            .fill_width()
            .any(),
    }
}

fn vector(reach: Reach, key: Target, values: Vec<f32>) -> Element {
    let rows = values.iter().enumerate().map(|(index, value)| {
        let (key, current) = (key.clone(), values.clone());
        row_of([
            text(CHANNELS[index]).caption().no_wrap().width(14.0).any(),
            number_field(*value)
                .step(0.01)
                .decimals(3)
                .control_size(ControlSize::Small)
                .on_commit(reach.act_with(move |studio, value: f32| {
                    let mut next = current.clone();
                    next[index] = value;
                    commit(studio, &key, &next);
                }))
                .fill_width()
                .any(),
        ])
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any()
    });
    column_of(rows).gap(Space::NONE).fill_width().any()
}

fn color(reach: Reach, key: Target, values: Vec<f32>, alpha: bool) -> Element {
    let encoded = Color::rgba(
        srgb::from_linear(values[0]),
        srgb::from_linear(values[1]),
        srgb::from_linear(values[2]),
        values.get(3).copied().unwrap_or(1.0),
    );
    let decode = move |color: Color| {
        let mut values = vec![
            srgb::to_linear(color.r),
            srgb::to_linear(color.g),
            srgb::to_linear(color.b),
        ];
        if alpha {
            values.push(color.a);
        }
        values
    };
    let (change, release) = (key.clone(), key);
    row_of([
        color_well(encoded)
            .alpha(alpha)
            .control_size(ControlSize::Small)
            .on_change(reach.act_with(move |studio, color: Color| {
                draft(studio, &change, &decode(color));
            }))
            .on_commit(reach.act_with(move |studio, color: Color| {
                commit(studio, &release, &decode(color));
            }))
            .any(),
        spacer().grow(1.0).any(),
    ])
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn asset(
    reach: Reach,
    key: Target,
    value: Option<SocketValue>,
    filter: crate::traits::AssetFilter,
) -> Element {
    let mut parts = Vec::new();
    match value.as_ref().filter(|value| value.asset().is_some()) {
        Some(held) => {
            parts.push(text(shown_value(held)).caption().no_wrap().grow(1.0).any());
            let cleared = key.clone();
            parts.push(
                glyph(MaterialSymbol::CLOSE)
                    .ghost()
                    .on_press(
                        reach.act(move |studio| studio.set_value(cleared.clone(), None, None)),
                    )
                    .tooltip(tr!("animate.shader.clear_file")),
            );
        }
        None => parts.push(
            text(tr!("animate.shader.no_file"))
                .caption()
                .muted()
                .grow(1.0)
                .any(),
        ),
    }
    parts.push(
        button(text(tr!("animate.shader.choose")).caption())
            .small()
            .ghost()
            .focusless()
            .on_press(reach.act(move |studio| studio.choose_asset(key.clone(), filter)))
            .any(),
    );
    row_of(parts)
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any()
}

fn written(reach: Reach, key: Target, document: Option<TextDocument>) -> Element {
    let Some(document) = document else {
        return text("").any();
    };
    let reading = document.clone();
    text_field(&document)
        .control_size(ControlSize::Small)
        .on_submit(reach.act(move |studio| {
            let written = reading.text().trim().to_owned();
            let value = (!written.is_empty()).then_some(SocketValue::Asset(written));
            studio.set_value(key.clone(), value, None);
            reading.mark_saved();
        }))
        .fill_width()
        .any()
}

fn control(
    reach: Reach,
    key: Target,
    field: &Field,
    value: Option<SocketValue>,
    document: Option<TextDocument>,
) -> Element {
    match field {
        Field::Number { .. } => {
            let at = constant(&value, 1)[0];
            number(reach, key, at, field)
        }
        Field::Vector(width) => vector(reach, key, constant(&value, *width)),
        Field::Color { alpha } => color(
            reach,
            key,
            constant(&value, if *alpha { 4 } else { 3 }),
            *alpha,
        ),
        Field::Toggle => {
            let on = constant(&value, 1)[0] > 0.5;
            switch(on)
                .control_size(ControlSize::Small)
                .on_toggle(reach.act_with(move |studio, on: bool| {
                    commit(studio, &key, &[if on { 1.0 } else { 0.0 }]);
                }))
                .any()
        }
        Field::Options(options) => {
            let chosen = constant(&value, 1)[0].round().max(0.0) as usize;
            combo(Some(chosen))
                .options(options.clone())
                .on_select(reach.act_with(move |studio, index: usize| {
                    commit(studio, &key, &[index as f32]);
                }))
                .focusless()
                .control_size(ControlSize::Small)
                .fill_width()
                .any()
        }
        Field::Asset(filter) => asset(reach, key, value, *filter),
        Field::Text => written(reach, key, document),
        Field::Link => text(tr!("animate.shader.link_only"))
            .caption()
            .muted()
            .any(),
    }
}

pub(crate) fn keyable(field: &Field) -> bool {
    !matches!(field, Field::Link)
}

pub(crate) fn value_row(reach: Reach, row: ValueRow, keying: KeyState) -> Element {
    let mut trailing = Vec::new();
    if keyable(&row.field) {
        let keyed = row.key.clone();
        let diamond = glyph(MaterialSymbol::DIAMOND)
            .on_press(reach.act(move |studio| studio.toggle_key(keyed.clone())));
        trailing.push(
            match keying {
                KeyState::Keyed => diamond.primary(),
                KeyState::Animated => diamond.secondary(),
                KeyState::Unkeyed => diamond.ghost(),
            }
            .tooltip(tr!("animate.shader.key")),
        );
    }
    let reset = row.key.clone();
    trailing.push(
        glyph(MaterialSymbol::RESTART_ALT)
            .ghost()
            .enabled(row.set)
            .on_press(reach.act(move |studio| studio.set_value(reset.clone(), None, None)))
            .tooltip(tr!("animate.shader.reset")),
    );
    labelled(
        row.label,
        control(reach, row.key, &row.field, row.value, row.document),
        Some(row_of(trailing).gap(Space::NONE).any()),
    )
}
