use glam::Vec3;

use crate::blueprint::{Blueprint, BodyDef, Geometry, Shape};
use crate::error::Result;
use crate::scene::{Driver, Setup};
use crate::size::Size;
use crate::world::World;

const CELL: f32 = 10.0;
const SPHERES: usize = 100;
const DROP_INTERVAL: u32 = 5;

struct Rain {
    grid: usize,
    ball: Shape,
    dropped: usize,
}

impl Driver for Rain {
    fn drive(&mut self, world: &mut dyn World, step: u32) -> Result<()> {
        if self.dropped >= SPHERES || step == 0 || !step.is_multiple_of(DROP_INTERVAL) {
            return Ok(());
        }
        let side = (SPHERES as f32).sqrt().ceil() as usize;
        let (gi, gj) = (self.dropped % side, self.dropped / side);
        let half = 0.5 * CELL * self.grid as f32;
        let inset = 0.2 * half;
        let usable = 2.0 * half - 2.0 * inset;
        let x = -half + inset + (gi as f32 + 0.5) * usable / side as f32;
        let z = -half + inset + (gj as f32 + 0.5) * usable / side as f32;
        world.add_body(
            &BodyDef::dynamic()
                .position(Vec3::new(x, 1.5, z))
                .shape(self.ball.clone()),
        )?;
        self.dropped += 1;
        Ok(())
    }
}

pub(super) fn create(size: Size) -> Result<Setup> {
    let grid = size.pick(1000, 50);
    let mut blueprint = Blueprint::new(true);
    let half = 0.5 * CELL * grid as f32;
    let tile = Shape::new(Geometry::cuboid(Vec3::new(0.5 * CELL, 0.25, 0.5 * CELL)));
    for i in 0..grid {
        for j in 0..grid {
            let x = -half + (i as f32 + 0.5) * CELL;
            let z = -half + (j as f32 + 0.5) * CELL;
            blueprint.add(
                BodyDef::fixed()
                    .position(Vec3::new(x, 0.0, z))
                    .shape(tile.clone()),
            );
        }
    }
    blueprint.reserve(SPHERES);
    Ok(Setup::new(blueprint).driven(Rain {
        grid,
        ball: Shape::new(Geometry::sphere(0.5)),
        dropped: 0,
    }))
}
