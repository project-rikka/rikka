use animate_body::{HandPreset, Side};
use animate_cast::{Grip, grip_of};
use animate_motion::PoseSample;
use animate_ui::{choice, heading, labelled};
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::cards::{card, holds};
use crate::{Desk, Listed, Reach, wording};

const FINGERS: usize = 5;

pub(super) fn hand(
    desk: Desk<'_>,
    pose_sample: &PoseSample,
    side: Side,
    hand_holds: &[Listed],
) -> Element {
    let current_preset = HandPreset::matching(side, &pose_sample.pose);
    let body = column! {
        row_of(HandPreset::ALL.map(|preset| {
            choice(wording::hand(preset), current_preset == Some(preset))
                .on_press(desk.act(move |studio| studio.hand_preset(side, preset)))
        }))
        .gap(Space::TIGHT)
        .wrap_items()
        .fill_width();
        rule();
        gripper(desk.reach(), pose_sample, side, Grip::Curl, tr!("animate.hand.curl"));
        gripper(desk.reach(), pose_sample, side, Grip::Spread, tr!("animate.hand.spread"));
        column_of((0..FINGERS).map(|finger| {
            gripper(desk.reach(), pose_sample, side, Grip::Finger(finger), animate_ui::finger_label(finger))
        }))
        .gap(Space::TIGHT)
        .fill_width();
        if !hand_holds.is_empty() {
            rule();
            heading(tr!("animate.hold.title"));
            holds::holds(desk, hand_holds);
        }
    }
    .gap(Space::SNUG)
    .fill_width();
    card(desk.reach(), animate_ui::holding_hand_label(side), body).build()
}

fn gripper(
    reach: Reach,
    pose_sample: &PoseSample,
    side: Side,
    grip: Grip,
    label: Label,
) -> impl View {
    labelled(
        label,
        slider(grip_of(pose_sample, side, grip))
            .range(-1.0..=1.0)
            .control_size(ControlSize::Small)
            .on_change(reach.act_with(move |studio, value: f32| {
                studio.regrip(side, grip, value);
            }))
            .fill_width(),
        None,
    )
}
