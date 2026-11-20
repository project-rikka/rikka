use animate_body::{Body, Expression, Faces, HumanPose, MorphWeights, Side};
use animate_motion::{GazeMode, PoseSample};
use animate_solve::EyeDirection;
use animate_ui::{choice, labelled, toggle};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::cards::pad::Pad;
use crate::cards::{amount, strength};
use crate::{Desk, Reach, wording};

const BOTH_HEIGHT: f32 = 132.0;
const EACH_HEIGHT: f32 = 112.0;

pub(super) fn eyes(
    desk: Desk<'_>,
    body: &Body,
    faces: &Faces,
    pose_sample: &PoseSample,
    weights: &MorphWeights,
) -> impl View {
    let gaze = pose_sample.gaze;
    let aimed = gaze.mode != GazeMode::Off;
    let visible: Vec<Side> = Side::BOTH
        .into_iter()
        .filter(|eye| body.has(eye.eye()))
        .collect();
    let split = desk.controls().split_eyes && visible.len() == Side::BOTH.len();
    let available = |expression: &Expression| faces.has(*expression);
    let pose = desk.shown_pose(body, pose_sample);
    let limits = desk.limits();
    column! {
        if let Some(first) = visible.first().copied() {
            pads(desk.reach(), &pose, first, split, limits);
            row! {
                container(toggle(
                    tr!("animate.gaze.each_eye"),
                    split,
                    desk.act_with(move |studio, on: bool| studio.controls_mut().split_eyes = on),
                ))
                .grow(1.0);
                choice(tr!("animate.gaze.ahead"), false)
                    .on_press(desk.act(move |studio| studio.eye_direction(None, EyeDirection::default())));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        segmented(GazeMode::ALL.iter().position(|mode| *mode == gaze.mode).unwrap_or(0))
            .segments(GazeMode::ALL.map(wording::gaze))
            .on_select(desk.act_with(move |studio, index: usize| {
                if let Some(mode) = GazeMode::ALL.get(index).copied() {
                    studio.aim_gaze(mode);
                }
            }))
            .focusless()
            .control_size(ControlSize::Small)
            .fill_width();
        if aimed {
            labelled(
                tr!("animate.gaze.weight"),
                amount(desk.reach(), gaze.weight, |pose_sample, value| pose_sample.gaze.weight = value),
                None,
            );
            labelled(
                tr!("animate.gaze.head_follow"),
                amount(desk.reach(), gaze.head_follow, |pose_sample, value| pose_sample.gaze.head_follow = value),
                None,
            );
        }
        column_of(Expression::BLINKS.into_iter().filter(available).map(|blink| {
            labelled(
                wording::expression(blink),
                strength(desk.reach(), blink, faces.strength(weights, blink)),
                None,
            )
        }))
        .gap(Space::TIGHT)
        .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn pads(reach: Reach, pose: &HumanPose, first: Side, split: bool, limits: bool) -> Element {
    if !split {
        return pad(reach, EyeDirection::from_pose(pose, first, limits), None);
    }
    row_of([Side::Right, Side::Left].map(|eye| {
        container(pad(
            reach,
            EyeDirection::from_pose(pose, eye, limits),
            Some(eye),
        ))
        .grow(1.0)
        .any()
    }))
    .gap(Space::TIGHT)
    .fill_width()
    .any()
}

fn pad(reach: Reach, eye_direction: EyeDirection, eye: Option<Side>) -> Element {
    let height = match eye {
        Some(_) => EACH_HEIGHT,
        None => BOTH_HEIGHT,
    };
    Element::new(Pad::new(
        eye_direction,
        eye.map(wording::eye),
        reach.act_with(move |studio, eye_direction: EyeDirection| {
            studio.eye_direction(eye, eye_direction)
        }),
    ))
    .height(height)
    .fill_width()
}
