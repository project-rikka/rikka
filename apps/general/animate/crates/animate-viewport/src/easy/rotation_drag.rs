use animate_body::HumanBone;
use animate_solve::Handle;
use rikka_app::prelude::*;
use rikka_model::glam::{Quat, Vec3};

use crate::easy::scene::Scene;
use crate::easy::style::TURN_PACE;
#[derive(Clone, Copy, Debug, PartialEq)]
enum RotationMode {
    Trackball {
        right: Vec3,
        up: Vec3,
        turned: Quat,
    },
    Turntable {
        upright: Vec3,
        across: Vec3,
        yawed: f32,
        pitched: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RotationDrag {
    last: Point,
    mode: RotationMode,
}

fn upright(scene: &Scene, handle: Handle) -> Option<Vec3> {
    let carrier = match handle {
        Handle::Centre | Handle::LowerBody => return Some(Vec3::Y),
        Handle::UpperBody => HumanBone::Hips,
        Handle::Head => scene
            .body
            .parent(HumanBone::Neck)
            .unwrap_or(HumanBone::Spine),
        _ => return None,
    };
    Some(scene.shown.rotation(carrier) * Vec3::Y)
}

impl RotationDrag {
    pub(super) fn new(scene: &Scene, handle: Handle, point: Vec3, press: Point) -> Self {
        let (right, up, _) = scene.lens.sideways(point);
        let mode = match upright(scene, handle) {
            Some(upright) => {
                let upright = match upright.dot(up) < 0.0 {
                    true => -upright,
                    false => upright,
                };
                RotationMode::Turntable {
                    upright,
                    across: right.reject_from_normalized(upright).normalize_or(right),
                    yawed: 0.0,
                    pitched: 0.0,
                }
            }
            None => RotationMode::Trackball {
                right,
                up,
                turned: Quat::IDENTITY,
            },
        };
        Self { last: press, mode }
    }

    pub(super) fn advance(&mut self, at: Point) -> Quat {
        let moved = at - self.last;
        self.last = at;
        match &mut self.mode {
            RotationMode::Trackball { right, up, turned } => {
                if let Some(axis) = (*right * moved.y + *up * moved.x).try_normalize() {
                    let step = Quat::from_axis_angle(axis, moved.x.hypot(moved.y) / TURN_PACE);
                    *turned = (step * *turned).normalize();
                }
                *turned
            }
            RotationMode::Turntable {
                upright,
                across,
                yawed,
                pitched,
            } => {
                *yawed += moved.x / TURN_PACE;
                *pitched += moved.y / TURN_PACE;
                (Quat::from_axis_angle(*across, *pitched) * Quat::from_axis_angle(*upright, *yawed))
                    .normalize()
            }
        }
    }
}
