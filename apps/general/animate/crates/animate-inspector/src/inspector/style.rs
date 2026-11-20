use animate_cast::Tuning;
use animate_ui::{heading, labelled, toggle};
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::{Desk, wording};

pub(super) fn tuned(desk: Desk<'_>, tuning: Tuning) -> impl View {
    labelled(
        wording::tuning(tuning),
        slider(desk.tuned(tuning))
            .range(tuning.range())
            .control_size(ControlSize::Small)
            .on_change(desk.act_with(move |studio, value: f32| {
                studio.controls_mut().automation_draft = Some((tuning, value));
            }))
            .on_commit(desk.act_with(move |studio, value: f32| {
                studio.apply_tuning(tuning, value);
            }))
            .fill_width(),
        None,
    )
}

pub(super) fn style(desk: Desk<'_>) -> impl View {
    let automation = desk.automation().unwrap_or_default();
    let header = row! {
        heading(tr!("animate.style.heading"));
        spacer().grow(1.0);
        button(text(tr!("animate.style.reset")).caption())
            .small()
            .ghost()
            .focusless()
            .on_press(desk.act(move |studio| studio.reset_style()));
    }
    .align_items(Align::Center)
    .fill_width();
    let fitting = toggle(
        tr!("animate.style.fitting"),
        automation.fitting,
        desk.act_with(move |studio, on: bool| {
            studio.automate(&mut |automation| automation.fitting = on)
        }),
    );
    let regions: &[Tuning] = match desk.is_easy_mode() {
        true => &[],
        false => &Tuning::REGIONS,
    };
    let tunings = Tuning::STYLE
        .iter()
        .chain(regions)
        .map(|tuning| tuned(desk, *tuning).any());
    column_of([header.any(), fitting.any()].into_iter().chain(tunings))
        .gap(Space::TIGHT)
        .fill_width()
}
