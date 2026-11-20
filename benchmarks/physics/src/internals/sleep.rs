use rikka_physics_geometry::Cuboid;
use rikka_physics_math::Vec3;
use rikka_physics_rigid::{Body, BodyId, FilterJoint, RigidWorld};

use super::WorkloadResult;
use super::rigid::stepped_scene;

const PYRAMIDS: usize = 10;
const BASE: usize = 20;
const EXTENT: f32 = 0.5;

fn pyramid(world: &mut RigidWorld, center_x: f32) -> rikka_physics_rigid::Result<(BodyId, BodyId)> {
    let (mut a, mut b) = (None, None);
    for i in 0..BASE {
        let y = (2.0 * i as f32 + 1.0) * EXTENT;
        for j in i..BASE {
            let x = (i as f32 + 1.0) * EXTENT + 2.0 * (j - i) as f32 * EXTENT + center_x;
            let body = world.create_body(
                Body::dynamic()
                    .position(Vec3::new(x, y, 0.0))
                    .sleep_threshold(1.0)
                    .shape(Cuboid::new(Vec3::splat(EXTENT))),
            )?;
            match (i, j) {
                (0, 0) => a = Some(body),
                (1, 1) => b = Some(body),
                _ => {}
            }
        }
    }
    Ok((a.expect("base body"), b.expect("second row body")))
}

pub(crate) fn create(threads: usize) -> WorkloadResult {
    stepped_scene(
        threads,
        true,
        |world| {
            let base_width = 2.0 * EXTENT * BASE as f32;
            let pitch = base_width + 8.0 * EXTENT;
            let span = pitch * (PYRAMIDS - 1) as f32;
            world.create_body(
                Body::fixed()
                    .position(Vec3::new(0.0, -1.0, 0.0))
                    .shape(Cuboid::new(Vec3::new(0.5 * span + base_width, 1.0, 4.0))),
            )?;
            (0..PYRAMIDS)
                .map(|i| pyramid(world, -0.5 * span + i as f32 * pitch - 0.5 * base_width))
                .collect::<rikka_physics_rigid::Result<Vec<_>>>()
        },
        |pairs, world, _| {
            for &(a, b) in pairs.iter() {
                if !world.is_awake(a)? {
                    let joint = world.create_joint(FilterJoint::new(a, b))?;
                    world.destroy_joint(joint)?;
                    break;
                }
            }
            Ok(())
        },
    )
}
