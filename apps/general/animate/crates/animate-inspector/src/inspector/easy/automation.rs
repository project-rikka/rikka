use animate_cast::Tuning;
use animate_ui::{heading, toggle};
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::Desk;
use crate::inspector::style::tuned;

pub(super) fn automation(desk: Desk<'_>) -> impl View {
    let automation = desk.automation().unwrap_or_default();
    column! {
        heading(tr!("animate.easy.automation"));
        tuned(desk, Tuning::Secondary);
        toggle(
            tr!("animate.easy.auto_blink"),
            automation.blink,
            desk.act_with(move |studio, on: bool| studio.automate(&mut |automation| automation.blink = on)),
        );
        toggle(
            tr!("animate.easy.limits"),
            automation.limits,
            desk.act_with(move |studio, on: bool| studio.automate(&mut |automation| automation.limits = on)),
        );
        toggle(
            tr!("animate.easy.motion_paths"),
            desk.controls().paths,
            desk.act_with(move |studio, on: bool| studio.controls_mut().paths = on),
        );
    }
    .gap(Space::TIGHT)
    .fill_width()
}
