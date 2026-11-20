use animate_cast::{Garment, Mode};
use animate_cloth::{Field, FieldKind, FieldSection, FieldUnit, Reading};
use animate_ui::{framed, labelled};
use rikka_app::prelude::*;
use rikka_physics_tailor::{Curve, PieceKey};

use crate::inspector::physics::depth::DepthCurve;
use crate::inspector::physics::tuning::{Tuned, knobs};
use crate::{Desk, Dial, wording};

const CURVE_HEIGHT: f32 = 72.0;

pub(super) fn garment(desk: Desk<'_>) -> Option<Element> {
    let piece = desk.chosen_piece()?;
    let garment = desk.garment(piece)?;
    let key = piece.key;
    let reset = button(text(tr!("animate.cloth.reset")))
        .small()
        .ghost()
        .focusless()
        .enabled(garment.detailed || !garment.tuning.is_neutral())
        .on_press(desk.act(move |studio| studio.reset_piece(key)));
    let mut parts = vec![
        row_of([
            text(piece.name.clone()).bold().no_wrap().any(),
            spacer().grow(1.0).any(),
            reset.any(),
        ])
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any(),
    ];
    match desk.mode() {
        Mode::Easy => parts.extend(knobs(desk, Tuned::Piece(key), &garment.tuning)),
        Mode::Detail => {
            for section in FieldSection::ALL {
                parts.push(
                    text(wording::style_section(section))
                        .caption()
                        .bold()
                        .no_wrap()
                        .any(),
                );
                parts.extend(
                    Field::for_section(section).map(|field| field_row(desk, key, &garment, field)),
                );
            }
        }
    }
    Some(framed(parts))
}

fn field_row(desk: Desk<'_>, key: PieceKey, garment: &Garment, field: Field) -> Element {
    let label = wording::style_field(field);
    match (field.kind(), field.read(&garment.style)) {
        (FieldKind::Curve, Reading::Curve(curve)) => labelled(
            label,
            Element::new(DepthCurve::new(
                curve,
                field.range(),
                desk.act_with(move |studio, curve: Curve| {
                    studio.set_field(key, field, Reading::Curve(curve));
                }),
            ))
            .height(CURVE_HEIGHT)
            .fill_width(),
            None,
        ),
        (FieldKind::Toggle, Reading::Toggle(on)) => labelled(
            label,
            row_of([
                switch(on)
                    .control_size(ControlSize::Small)
                    .on_toggle(desk.act_with(move |studio, on: bool| {
                        studio.set_field(key, field, Reading::Toggle(on));
                    }))
                    .any(),
                spacer().grow(1.0).any(),
            ])
            .fill_width()
            .any(),
            None,
        ),
        (FieldKind::Optional, Reading::Optional(optional)) => {
            let fallback = *field.range().end() * 0.5;
            let toggle = switch(optional.is_some())
                .control_size(ControlSize::Small)
                .on_toggle(desk.act_with(move |studio, on: bool| {
                    let value = on.then_some(optional.unwrap_or(fallback));
                    studio.set_field(key, field, Reading::Optional(value));
                }))
                .any();
            let mut parts = vec![toggle];
            if let Some(value) = optional {
                parts.push(
                    container(number(desk, key, field, value, true))
                        .grow(1.0)
                        .any(),
                );
            }
            labelled(
                label,
                row_of(parts)
                    .gap(Space::TIGHT)
                    .align_items(Align::Center)
                    .fill_width()
                    .any(),
                None,
            )
        }
        (_, reading) => {
            let value = match reading {
                Reading::Number(value) => value,
                _ => *field.range().start(),
            };
            labelled(label, number(desk, key, field, value, false), None)
        }
    }
}

fn number(desk: Desk<'_>, key: PieceKey, field: Field, value: f32, optional: bool) -> Element {
    let display_value = |value: f32| match field.unit() {
        FieldUnit::Plain => value,
        FieldUnit::Radians => value.to_degrees(),
    };
    let stored = move |value: f32| match field.unit() {
        FieldUnit::Plain => value,
        FieldUnit::Radians => value.to_radians(),
    };
    let reading = move |value: f32| match optional {
        true => Reading::Optional(Some(stored(value))),
        false => Reading::Number(stored(value)),
    };
    let range = field.range();
    let (low, high) = (display_value(*range.start()), display_value(*range.end()));
    let dial = Dial::Field(field);
    let value = desk
        .cloth_draft(dial)
        .unwrap_or_else(|| display_value(value));
    slider(value.clamp(low, high))
        .range(low..=high)
        .control_size(ControlSize::Small)
        .on_change(desk.act_with(move |studio, value: f32| {
            studio.draft_cloth(dial, value);
        }))
        .on_commit(desk.act_with(move |studio, value: f32| {
            studio.set_field(key, field, reading(value));
        }))
        .fill_width()
        .any()
}
