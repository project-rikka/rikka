mod curve;
mod easy;
mod liquid;
mod morphs;
mod physics;
mod presets;
mod style;
mod wind;

use animate_cast::Subject;
use rikka_app::prelude::*;
use rikka_model::mmd::VmdCurve;
use rikka_widgets::{column, row};

use crate::{Desk, Pane, wording};
use curve::CurveEditor;

pub(crate) const WIDTH: f32 = 340.0;
const EDITOR: f32 = 240.0;
const CHANNEL_WIDTH: f32 = 120.0;

pub fn inspector(desk: Desk<'_>, node: impl FnOnce() -> Element) -> impl View {
    let pane = desk.controls().pane;
    let chosen = Pane::ALL
        .iter()
        .position(|candidate| *candidate == pane)
        .unwrap_or(0);
    let tabs = segmented(chosen)
        .segments(Pane::ALL.map(wording::pane))
        .control_size(ControlSize::Small)
        .on_select(desk.act_with(move |studio, index: usize| {
            if let Some(pane) = Pane::ALL.get(index) {
                studio.show_pane(*pane);
            }
        }))
        .fill_width();
    let body = match pane {
        Pane::Motion => scrolled(motion(desk)),
        Pane::Node => node(),
        Pane::Physics => scrolled(physics::physics(desk)),
    };
    let page = page_stack(chosen, body).grow(1.0).fill_width();
    container(
        column_of([
            container(tabs).padding(Inset::all(12.0)).fill_width().any(),
            rule().any(),
            page.any(),
        ])
        .fill(),
    )
    .background(palette().surface)
    .width(WIDTH)
    .fill_height()
}

fn scrolled(body: Element) -> Element {
    scrollable(container(body).padding(Inset::all(12.0)).fill_width())
        .fill()
        .any()
}

fn motion(desk: Desk<'_>) -> Element {
    if desk.subject() == Some(Subject::Wind) {
        return column! {
            wind::wind(desk);
            rule();
            detail(desk);
        }
        .gap(Space::SNUG)
        .fill_width()
        .any();
    }
    if let Some(emitter) = desk.chosen_emitter() {
        return column! {
            liquid::liquid(desk, emitter);
            rule();
            detail(desk);
        }
        .gap(Space::SNUG)
        .fill_width()
        .any();
    }
    if desk.is_easy_mode() {
        return easy::easy(desk).any();
    }
    column! {
        if desk.automation().is_some() {
            style::style(desk);
            rule();
        }
        if let Some(morphs) = morphs::morphs(desk) {
            morphs;
            rule();
        }
        detail(desk);
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn detail(desk: Desk<'_>) -> impl View {
    let shown_curve = desk.shown_curve();
    column! {
        text(tr!("animate.inspector.interpolation")).bold();
        row! {
            channel_picker(desk);
            spacer().grow(1.0);
            text(tr!("animate.inspector.linked")).caption().no_wrap();
            switch(desk.controls().linked)
                .control_size(ControlSize::Small)
                .on_toggle(desk.act_with(move |studio, linked| studio.controls_mut().linked = linked));
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width();
        Element::new(CurveEditor::new(
            shown_curve,
            desk.act_with(move |studio, curve: VmdCurve| studio.bend(curve)),
        ))
        .width(EDITOR)
        .height(EDITOR + 20.0)
        .align_self(Align::Center);
        rule();
        text(tr!("animate.inspector.presets")).bold();
        presets::presets(desk.reach(), shown_curve);
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn channel_picker(desk: Desk<'_>) -> impl View {
    let channels = desk.channels();
    let chosen = channels
        .iter()
        .position(|channel| *channel == desk.channel());
    combo(chosen)
        .options(channels.iter().map(|channel| wording::channel(*channel)))
        .on_select(desk.act_with(move |studio, index| studio.choose_channel(index)))
        .enabled(!desk.controls().linked)
        .focusless()
        .control_size(ControlSize::Small)
        .width(CHANNEL_WIDTH)
}
