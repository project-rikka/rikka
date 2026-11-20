use std::f32::consts::PI;

use glam::{Quat, Vec3};

use crate::blueprint::{Blueprint, BodyDef, Geometry};
use crate::error::Result;
use crate::scene::Setup;
use crate::size::Size;

const CUBE: f32 = 0.2;

fn slab(near: Vec3, far: Vec3, a: Vec3, b: Vec3, inner: f32, outer: f32) -> Result<Geometry> {
    Geometry::hull(vec![
        near + a * inner,
        near + a * outer,
        near + b * inner,
        near + b * outer,
        far + a * inner,
        far + a * outer,
        far + b * inner,
        far + b * outer,
    ])
}

pub(super) fn create(size: Size) -> Result<Setup> {
    let grid = size.pick(20, 5);
    let mut blueprint = Blueprint::new(true);
    blueprint.add(
        BodyDef::fixed()
            .position(Vec3::new(0.0, -1.0, 0.0))
            .shape(Geometry::cuboid(Vec3::new(60.0, 1.0, 60.0))),
    );
    let (r0, r1, r2) = (14.0, 16.0, 18.0);
    let near = Vec3::new(0.0, 0.0, -10.0);
    let far = Vec3::new(0.0, 0.0, 10.0);
    let step = Quat::from_rotation_z(PI / 18.0);
    let overlap = Quat::from_rotation_z(0.1 * PI / 18.0);
    let mut drum = BodyDef::kinematic()
        .position(Vec3::new(0.0, 21.0, 0.0))
        .velocity(
            Vec3::new(0.001, -0.002, 0.0),
            Vec3::new(0.0, 0.0, 25.0f32.to_radians()),
        );
    let mut u1 = Vec3::X;
    for i in 0..36 {
        let u2 = if i == 35 { Vec3::X } else { step * u1 };
        let a1 = overlap.inverse() * u1;
        let a2 = overlap * u2;
        drum = drum.shape(slab(near, far, a1, a2, r1, r2)?);
        if i % 9 == 0 {
            drum = drum.shape(slab(near, far, u1, u2, r0, r1)?);
        }
        u1 = u2;
    }
    blueprint.add(drum);
    let cube = Geometry::cuboid(Vec3::splat(CUBE));
    let start = -2.0 * CUBE * grid as f32;
    for i in 0..grid {
        for j in 0..grid {
            for k in 0..grid {
                let position = Vec3::new(
                    start + 4.0 * CUBE * i as f32,
                    start + 21.0 + 4.0 * CUBE * j as f32,
                    start + 4.0 * CUBE * k as f32,
                );
                blueprint.add(BodyDef::dynamic().position(position).shape(cube.clone()));
            }
        }
    }
    Ok(Setup::new(blueprint))
}
