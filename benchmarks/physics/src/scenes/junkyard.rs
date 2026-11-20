use glam::{Quat, Vec3};
use rikka_physics_math::cos_sin;

use super::forms;
use crate::blueprint::{Blueprint, BodyDef, BodyIndex, Geometry, Pose, Shape, TIME_STEP};
use crate::error::Result;
use crate::scene::{Driver, Setup};
use crate::size::Size;
use crate::world::World;

const RADIUS: f32 = 35.0;
const OMEGA: f32 = -6.0;

struct Pusher {
    body: BodyIndex,
    degrees: f32,
}

impl Driver for Pusher {
    fn drive(&mut self, world: &mut dyn World, _step: u32) -> Result<()> {
        self.degrees += OMEGA * TIME_STEP;
        let angle = cos_sin(self.degrees.to_radians());
        let target = Vec3::new(RADIUS * angle.cosine, 0.0, RADIUS * angle.sine);
        world.move_kinematic(self.body, Pose::new(target, Quat::IDENTITY));
        Ok(())
    }
}

pub(super) fn create(size: Size) -> Result<Setup> {
    let (layers, side) = size.pick((24, 20), (2, 4));
    let mut blueprint = Blueprint::new(true);
    let walls = [
        (Vec3::new(120.0, 1.0, 120.0), Vec3::ZERO),
        (Vec3::new(1.0, 8.0, 50.0), Vec3::new(-50.0, 8.0, 0.0)),
        (Vec3::new(1.0, 8.0, 50.0), Vec3::new(50.0, 8.0, 0.0)),
        (Vec3::new(50.0, 8.0, 1.0), Vec3::new(0.0, 8.0, -50.0)),
        (Vec3::new(50.0, 8.0, 1.0), Vec3::new(0.0, 8.0, 50.0)),
    ];
    let mut ground = BodyDef::fixed().position(Vec3::new(0.0, -1.0, 0.0));
    for (half, offset) in walls {
        ground =
            ground.shape(Shape::new(Geometry::cuboid(half)).placed(Pose::from_position(offset)));
    }
    blueprint.add(ground);
    let rock = forms::rock(1.5)?;
    for y in 0..layers {
        for x in 0..=side {
            for z in 0..=side {
                let position = Vec3::new(
                    -40.0 + 4.0 * x as f32,
                    4.0 * y as f32 + 25.0,
                    -40.0 + 4.0 * z as f32,
                );
                blueprint.add(BodyDef::dynamic().position(position).shape(rock.clone()));
            }
        }
    }
    let body = blueprint.add(
        BodyDef::kinematic()
            .position(Vec3::new(RADIUS, 0.0, 0.0))
            .shape(forms::cylinder(24.0, 4.0, 0.0, 16)?),
    );
    Ok(Setup::new(blueprint).driven(Pusher { body, degrees: 0.0 }))
}
