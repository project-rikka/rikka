use glam::Vec3;

use crate::blueprint::{Blueprint, BodyDef, BodyIndex, Geometry, Shape};
use crate::error::Result;
use crate::scene::Setup;
use crate::size::Size;

pub(super) fn create(size: Size) -> Result<Setup> {
    let side = size.pick(100, 10);
    let mut blueprint = Blueprint::new(false);
    let ball = Shape::new(Geometry::sphere(0.4)).filter(2, !2);
    let mut bodies: Vec<BodyIndex> = Vec::with_capacity(side * side);
    for k in 0..side {
        for i in 0..side {
            let body = match i {
                0 => BodyDef::fixed(),
                _ => BodyDef::dynamic().sleep(false),
            };
            let body = blueprint.add(
                body.position(Vec3::new(k as f32, -(i as f32), 0.0))
                    .shape(ball.clone()),
            );
            if i > 0 {
                blueprint.spherical(
                    bodies[k * side + i - 1],
                    body,
                    Vec3::new(0.0, -0.5, 0.0),
                    Vec3::new(0.0, 0.5, 0.0),
                );
            }
            if k > 0 && i > 0 {
                blueprint.spherical(
                    bodies[(k - 1) * side + i],
                    body,
                    Vec3::new(0.5, 0.0, 0.0),
                    Vec3::new(-0.5, 0.0, 0.0),
                );
            }
            bodies.push(body);
        }
    }
    Ok(Setup::new(blueprint))
}
