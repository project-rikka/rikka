use glam::Vec3;

use crate::blueprint::{Blueprint, BodyDef, Geometry, Shape};
use crate::error::Result;
use crate::scene::Setup;
use crate::size::Size;

const EXTENT: f32 = 0.5;

pub(super) fn large(size: Size) -> Result<Setup> {
    let base = size.pick(100, 20);
    let mut blueprint = Blueprint::new(false);
    blueprint.add(
        BodyDef::fixed()
            .position(Vec3::new(0.0, -1.0, 0.0))
            .shape(Geometry::cuboid(Vec3::new(400.0, 1.0, 400.0))),
    );
    let cube = Shape::new(Geometry::cuboid(Vec3::splat(EXTENT))).density(100.0);
    for i in 0..base {
        let y = (2.0 * i as f32 + 1.0) * EXTENT;
        for j in i..base {
            let x =
                (i as f32 + 1.0) * EXTENT + 2.0 * (j - i) as f32 * EXTENT - EXTENT * base as f32;
            blueprint.add(
                BodyDef::dynamic()
                    .position(Vec3::new(x, y, 0.0))
                    .shape(cube.clone()),
            );
        }
    }
    Ok(Setup::new(blueprint))
}

pub(super) fn many(size: Size) -> Result<Setup> {
    let base = 10;
    let (rows, columns) = size.pick((14, 14), (3, 3));
    let ground = EXTENT * columns as f32 * (base as f32 + 1.0);
    let mut blueprint = Blueprint::new(true);
    blueprint.add(
        BodyDef::fixed()
            .position(Vec3::new(0.0, -1.0, 0.0))
            .shape(Geometry::cuboid(Vec3::new(ground, 1.0, ground))),
    );
    let cube = Shape::new(Geometry::cuboid(Vec3::splat(EXTENT))).density(100.0);
    let width = 2.0 * EXTENT * base as f32;
    let mut base_z = -ground + 2.0 * EXTENT;
    let delta_z = 2.0 * (ground - 2.0 * EXTENT) / (rows as f32 - 1.0);
    for _ in 0..rows {
        for column in 0..columns {
            let center_x = -ground + column as f32 * (width + 2.0 * EXTENT) + 2.0 * EXTENT;
            for i in 0..base {
                let y = (2.0 * i as f32 + 1.0) * EXTENT;
                for j in i..base {
                    let x =
                        (i as f32 + 1.0) * EXTENT + 2.0 * (j - i) as f32 * EXTENT + center_x - 0.5;
                    blueprint.add(
                        BodyDef::dynamic()
                            .position(Vec3::new(x, y, base_z))
                            .sleep(false)
                            .shape(cube.clone()),
                    );
                }
            }
        }
        base_z += delta_z;
    }
    Ok(Setup::new(blueprint))
}
