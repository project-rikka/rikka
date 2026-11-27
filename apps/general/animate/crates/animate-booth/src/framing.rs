use animate_cast::{Orbit, Shot};
use rikka_model::Bounds;
use rikka_model::glam::{EulerRot, Quat, Vec3};

const FOV_DEGREES: f32 = 30.0;
const MARGIN: f32 = 1.06;
const NEAREST: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fit {
    Silhouette,
    Enclosing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Aim {
    pub(crate) yaw: f32,
    pub(crate) elevation: f32,
    pub(crate) fit: Fit,
}

impl Aim {
    pub(crate) fn facing(forward: Vec3, turn: f32, elevation: f32, fit: Fit) -> Self {
        let front = forward.x.atan2(forward.z).to_degrees();
        Self {
            yaw: front + turn,
            elevation,
            fit,
        }
    }
}

pub(crate) fn fit(bounds: Bounds, aim: Aim, aspect: f32) -> Shot {
    let target = bounds.center();
    let rotation = Quat::from_euler(
        EulerRot::YXZ,
        aim.yaw.to_radians(),
        -aim.elevation.to_radians(),
        0.0,
    );
    let unturn = rotation.inverse();
    let vertical = (FOV_DEGREES.to_radians() * 0.5).tan() / MARGIN;
    let horizontal = vertical * aspect;
    let mut distance = NEAREST;
    for corner in 0..8 {
        let point = Vec3::new(
            [bounds.min.x, bounds.max.x][corner & 1],
            [bounds.min.y, bounds.max.y][(corner >> 1) & 1],
            [bounds.min.z, bounds.max.z][(corner >> 2) & 1],
        );
        let local = unturn * (point - target);
        let nearer = match aim.fit {
            Fit::Silhouette => 0.0,
            Fit::Enclosing => local.z,
        };
        distance = distance
            .max(nearer + local.x.abs() / horizontal)
            .max(nearer + local.y.abs() / vertical)
            .max(local.z + NEAREST);
    }
    Shot::placed(
        target,
        Orbit {
            yaw: aim.yaw,
            elevation: aim.elevation,
            roll: 0.0,
            distance,
            fov: FOV_DEGREES,
        },
    )
}
