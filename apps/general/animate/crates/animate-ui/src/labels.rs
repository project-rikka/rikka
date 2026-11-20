use std::error::Error;

use animate_body::Side;
use animate_fluid::EmitterSetting;
use animate_motion::Part;
use animate_wind::WindSetting;
use rikka_i18n::{Label, tr};
use rikka_model::mmd::FRAMES_PER_SECOND;

pub fn frame_clock(frames: u32) -> String {
    let per_second = FRAMES_PER_SECOND as u32;
    let seconds = frames / per_second;
    format!(
        "{}:{:02}:{:02}",
        seconds / 60,
        seconds % 60,
        frames % per_second
    )
}

pub fn error_chain(error: &dyn Error) -> String {
    let mut parts = vec![error.to_string()];
    let mut source = error.source();
    while let Some(cause) = source {
        parts.push(cause.to_string());
        source = cause.source();
    }
    parts.join(": ")
}

pub fn part_label(part: Part) -> Label {
    match part {
        Part::Root => tr!("animate.part.root"),
        Part::Torso => tr!("animate.part.torso"),
        Part::Head => tr!("animate.part.head"),
        Part::LeftArm => tr!("animate.part.left_arm"),
        Part::RightArm => tr!("animate.part.right_arm"),
        Part::LeftFingers => tr!("animate.part.left_fingers"),
        Part::RightFingers => tr!("animate.part.right_fingers"),
        Part::LeftLeg => tr!("animate.part.left_leg"),
        Part::RightLeg => tr!("animate.part.right_leg"),
        Part::Gaze => tr!("animate.part.gaze"),
    }
}

pub fn emitter_setting_label(setting: EmitterSetting) -> Label {
    tr!(&format!("animate.liquid.setting.{}", setting.id()))
}

pub fn wind_setting_label(setting: WindSetting) -> Label {
    tr!(&format!("animate.wind.setting.{}", setting.id()))
}

pub fn holding_hand_label(side: Side) -> Label {
    match side {
        Side::Left => tr!("animate.hold.left_hand"),
        Side::Right => tr!("animate.hold.right_hand"),
    }
}

pub fn side_label(side: Side) -> Label {
    match side {
        Side::Left => tr!("animate.hand.left"),
        Side::Right => tr!("animate.hand.right"),
    }
}

pub fn finger_label(finger: usize) -> Label {
    match finger {
        0 => tr!("animate.hand.thumb"),
        1 => tr!("animate.hand.index"),
        2 => tr!("animate.hand.middle"),
        3 => tr!("animate.hand.ring"),
        _ => tr!("animate.hand.little"),
    }
}
