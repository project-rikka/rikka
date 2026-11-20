use glam::Vec3;

use super::forms;
use crate::blueprint::{Blueprint, BodyDef, Geometry, Motor, Pose, Shape};
use crate::error::Result;
use crate::scene::Setup;
use crate::size::Size;

const SEGMENTS: u32 = 90;
const RADIUS: f32 = 12.0;
const HALF_DEPTH: f32 = 1.0;

pub(super) fn create(size: Size) -> Result<Setup> {
    let balls = size.pick(1500, 100);
    let mut blueprint = Blueprint::new(true);
    let y_center = RADIUS;
    let ground = blueprint.add(
        BodyDef::fixed().shape(
            Shape::new(forms::inward_cylinder(
                SEGMENTS, RADIUS, HALF_DEPTH, y_center,
            ))
            .friction(0.1),
        ),
    );
    let half_length = 0.5 * RADIUS;
    let pivot = Vec3::new(0.0, y_center - half_length, 0.0);
    let spinner = blueprint.add(
        BodyDef::dynamic().position(pivot).sleep(false).shape(
            Shape::new(Geometry::cuboid(Vec3::new(
                0.3,
                half_length + 0.1,
                HALF_DEPTH + 0.1,
            )))
            .friction(0.0),
        ),
    );
    blueprint.revolute(
        ground,
        spinner,
        Pose::from_position(pivot),
        Some(Motor {
            speed: 8.0,
            max_torque: f32::MAX,
        }),
    );
    let ball = Shape::new(Geometry::sphere(0.2))
        .friction(0.1)
        .restitution(0.1)
        .density(250.0);
    let spacing = 0.55;
    let y_start = 2.0;
    let chord = (RADIUS * RADIUS - (y_center - y_start) * (y_center - y_start)).sqrt();
    let x_limit = chord - 2.0 * spacing;
    let z_limit = HALF_DEPTH - 2.0 * 0.2;
    let (mut x, mut y, mut z) = (-x_limit, y_start, -z_limit);
    for _ in 0..balls {
        blueprint.add(
            BodyDef::dynamic()
                .position(Vec3::new(x, y, z))
                .shape(ball.clone()),
        );
        z += spacing;
        if z > z_limit {
            z = -z_limit;
            x += spacing;
            if x > x_limit {
                x = -x_limit;
                y += spacing;
            }
        }
    }
    Ok(Setup::new(blueprint))
}
