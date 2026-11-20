use animate_fluid::{Emitter, Pour};
use animate_ui::{heading, labelled};
use rikka_app::prelude::*;
use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_ui::IntoHandler;

use crate::{Desk, Reach, wording};

const SECOND_STEP: f32 = 0.1;

fn seconds(frames: u32) -> f32 {
    frames as f32 / FRAMES_PER_SECOND
}

pub(super) fn rows(desk: Desk<'_>, emitter: &Emitter) -> Vec<Element> {
    let pour = emitter.pour;
    let frame = desk.transport().frame();
    let mut parts = vec![
        heading(tr!("animate.liquid.pouring")),
        labelled(
            tr!("animate.liquid.start"),
            start(desk.reach(), pour),
            Some(to_playhead(
                tr!("animate.liquid.start_here"),
                pour.can_start_at(frame),
                desk.act(move |studio| studio.start_pour_here()),
            )),
        ),
    ];
    if emitter.shape.flows() {
        parts.push(labelled(
            tr!("animate.liquid.duration"),
            duration(desk.reach(), pour),
            Some(to_playhead(
                tr!("animate.liquid.end_here"),
                pour.can_end_at(frame),
                desk.act(move |studio| studio.end_pour_here()),
            )),
        ));
    }
    parts.push(labelled(
        Label::default(),
        text(summary(emitter)).caption().muted().no_wrap().any(),
        None,
    ));
    parts
}

fn start(reach: Reach, pour: Pour) -> Element {
    number_field(pour.start() as f32)
        .range(*Pour::STARTS.start() as f32..=*Pour::STARTS.end() as f32)
        .unit(tr!("animate.liquid.frame_unit"))
        .control_size(ControlSize::Small)
        .on_commit(reach.act_with(move |studio, frame: f32| studio.pour_from(frame.round() as u32)))
        .fill_width()
        .any()
}

fn duration(reach: Reach, pour: Pour) -> Element {
    number_field(pour.seconds())
        .range(seconds(*Pour::LENGTHS.start())..=seconds(*Pour::LENGTHS.end()))
        .step(SECOND_STEP)
        .decimals(2)
        .unit(tr!("animate.liquid.second_unit"))
        .control_size(ControlSize::Small)
        .on_commit(reach.act_with(move |studio, seconds: f32| studio.pour_for(seconds)))
        .fill_width()
        .any()
}

fn to_playhead<Marker>(
    label: Label,
    enabled: bool,
    press: impl IntoHandler<(), (), Marker>,
) -> Element {
    button(text(label))
        .small()
        .ghost()
        .focusless()
        .enabled(enabled)
        .on_press(press)
        .any()
}

fn summary(emitter: &Emitter) -> Label {
    let pour = emitter.pour;
    let litres = wording::litres(emitter.litres());
    match emitter.shape.flows() {
        true => tr!(
            "animate.liquid.span",
            from = animate_ui::frame_clock(pour.start()).as_str(),
            to = animate_ui::frame_clock(pour.end()).as_str(),
            litres = litres.as_str()
        ),
        false => tr!(
            "animate.liquid.landing",
            at = animate_ui::frame_clock(pour.start()).as_str(),
            litres = litres.as_str()
        ),
    }
}
