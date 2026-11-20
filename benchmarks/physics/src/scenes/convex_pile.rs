use glam::Vec3;

use crate::blueprint::{Blueprint, BodyDef, Geometry};
use crate::error::Result;
use crate::scene::Setup;
use crate::size::Size;

const COUNT_X: usize = 8;
const COUNT_Z: usize = 8;
const AMPLITUDE: f32 = 2.0;
const POINTS: usize = 32;

struct Random(u32);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(2_147_001_325).wrapping_add(715_136_305);
        self.0
    }

    fn unit(&mut self) -> f32 {
        (self.next() & 0xffff) as f32 / 65535.0 - 0.5
    }

    fn direction(&mut self) -> Vec3 {
        loop {
            let point = Vec3::new(self.unit(), self.unit(), self.unit());
            if point.length_squared() <= 0.25 {
                return point.normalize();
            }
        }
    }
}

pub(super) fn create(size: Size) -> Result<Setup> {
    let layers = size.pick(80, 10);
    let mut blueprint = Blueprint::new(true);
    blueprint.add(
        BodyDef::fixed()
            .position(Vec3::new(0.0, -1.0, 0.0))
            .shape(Geometry::cuboid(Vec3::new(250.0, 1.0, 250.0))),
    );
    let mut random = Random(42);
    let points = (0..POINTS)
        .map(|_| random.direction() * AMPLITUDE)
        .collect();
    let hull = Geometry::hull(points)?;
    let scatter = 2.0 * AMPLITUDE;
    for layer in 0..layers {
        for z in 0..COUNT_Z {
            for x in 0..COUNT_X {
                let position = Vec3::new(
                    (x as f32 - 0.5 * COUNT_X as f32) * scatter,
                    AMPLITUDE + 2.0 * AMPLITUDE * layer as f32,
                    (z as f32 - 0.5 * COUNT_Z as f32) * scatter,
                );
                blueprint.add(BodyDef::dynamic().position(position).shape(hull.clone()));
            }
        }
    }
    Ok(Setup::new(blueprint))
}
