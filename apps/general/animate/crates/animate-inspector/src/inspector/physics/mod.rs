mod depth;
mod garment;
mod pieces;
mod tuning;

use animate_cast::Figure;
use animate_cloth::{Brush, Tool};
use animate_ui::{framed, glyph, labelled};
use rikka_app::prelude::*;
use rikka_physics_tailor::Quality;

use crate::{Desk, Reach, wording};

const GLYPH: f32 = 16.0;
const QUALITIES: [Quality; 3] = [Quality::Low, Quality::Medium, Quality::High];

pub(super) fn physics(desk: Desk<'_>) -> Element {
    let mut parts = vec![simulation(desk)];
    if let Some(figure) = desk.chosen_figure() {
        parts.push(header(desk, figure));
        parts.extend(tuning::figure(desk));
        parts.push(pieces::pieces(desk, figure));
        parts.extend(garment::garment(desk));
        parts.push(painting(desk));
    }
    column_of(parts).gap(Space::SNUG).fill_width().any()
}

fn simulation(desk: Desk<'_>) -> Element {
    let on = desk.is_physics_enabled();
    let reset = action(MaterialSymbol::RESTART_ALT, tr!("animate.physics.reset"))
        .ghost()
        .enabled(desk.is_physics_enabled() && !desk.cast().figures().is_empty())
        .on_press(desk.act(move |studio| studio.reset_physics()));
    framed(vec![
        row_of([
            text(tr!("animate.physics.title")).bold().no_wrap().any(),
            spacer().grow(1.0).any(),
            reset.any(),
            switch(on)
                .control_size(ControlSize::Small)
                .on_toggle(desk.act_with(move |studio, on: bool| studio.set_physics(on)))
                .any(),
        ])
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any(),
        quality(desk),
    ])
}

fn header(desk: Desk<'_>, figure: &Figure) -> Element {
    let save = action(MaterialSymbol::SAVE, tr!("animate.cloth.save"))
        .enabled(desk.can_save_cloth())
        .on_press(desk.act(move |studio| studio.save_cloth()));
    row_of([
        text(figure.name.clone()).bold().no_wrap().any(),
        spacer().grow(1.0).any(),
        save.any(),
    ])
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn quality(desk: Desk<'_>) -> Element {
    let chosen = QUALITIES
        .iter()
        .position(|quality| *quality == desk.cloth_quality());
    labelled(
        tr!("animate.cloth.quality.title"),
        segmented(chosen.unwrap_or(QUALITIES.len() - 1))
            .segments(QUALITIES.map(wording::quality))
            .control_size(ControlSize::Small)
            .focusless()
            .on_select(desk.act_with(move |studio, index: usize| {
                if let Some(quality) = QUALITIES.get(index) {
                    studio.set_cloth_quality(*quality);
                }
            }))
            .fill_width()
            .any(),
        None,
    )
}

fn painting(desk: Desk<'_>) -> Element {
    let open = desk.controls().painting;
    let toggle = glyph(match open {
        true => MaterialSymbol::EXPAND_MORE,
        false => MaterialSymbol::CHEVRON_RIGHT,
    })
    .ghost()
    .on_press(desk.act(move |studio| studio.show_painting(!open)));
    let mut heading = vec![
        toggle.any(),
        text(tr!("animate.physics.painting")).bold().no_wrap().any(),
        spacer().grow(1.0).any(),
    ];
    if open {
        heading.push(
            action(MaterialSymbol::RESTART_ALT, tr!("animate.cloth.redetect"))
                .ghost()
                .on_press(desk.act(move |studio| studio.redetect()))
                .any(),
        );
    }
    let mut parts = vec![
        row_of(heading)
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width()
            .any(),
    ];
    if open {
        parts.extend(brush(desk));
    }
    framed(parts)
}

fn brush(desk: Desk<'_>) -> Vec<Element> {
    let current = desk.controls().brush;
    let chosen = Tool::ALL.iter().position(|tool| *tool == current.tool);
    let mut parts = vec![
        segmented(chosen.unwrap_or(0))
            .segments(Tool::ALL.map(wording::brush))
            .control_size(ControlSize::Small)
            .focusless()
            .on_select(desk.act_with(move |studio, index: usize| {
                if let Some(tool) = Tool::ALL.get(index) {
                    studio.controls_mut().brush.tool = *tool;
                }
            }))
            .fill_width()
            .any(),
        amount(
            desk.reach(),
            tr!("animate.cloth.radius"),
            current.radius,
            Brush::RADII,
            |brush, value| brush.radius = value,
        ),
        labelled(
            tr!("animate.cloth.mirror"),
            row_of([
                switch(current.mirror)
                    .control_size(ControlSize::Small)
                    .on_toggle(desk.act_with(move |studio, on: bool| {
                        studio.controls_mut().brush.mirror = on;
                    }))
                    .any(),
                spacer().grow(1.0).any(),
            ])
            .fill_width()
            .any(),
            None,
        ),
    ];
    if current.tool == Tool::Spread {
        parts.push(amount(
            desk.reach(),
            tr!("animate.cloth.spread"),
            current.spread,
            Brush::SPREADS,
            |brush, value| brush.spread = value,
        ));
        parts.push(amount(
            desk.reach(),
            tr!("animate.cloth.strength"),
            current.strength,
            Brush::STRENGTHS,
            |brush, value| brush.strength = value,
        ));
    }
    parts
}

fn action(symbol: MaterialSymbol, label: Label) -> Button {
    button(
        row_of([
            icon(symbol).extent(Length::px(GLYPH)).any(),
            text(label).no_wrap().any(),
        ])
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .small()
    .focusless()
}

fn amount(
    reach: Reach,
    label: Label,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    set: fn(&mut Brush, f32),
) -> Element {
    labelled(
        label,
        slider(value.clamp(*range.start(), *range.end()))
            .range(range)
            .control_size(ControlSize::Small)
            .on_change(reach.act_with(move |studio, value: f32| {
                set(&mut studio.controls_mut().brush, value);
            }))
            .fill_width()
            .any(),
        None,
    )
}
