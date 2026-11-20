use glam::Vec3;

use super::forms;
use crate::blueprint::{Blueprint, BodyDef, Shape};
use crate::error::Result;
use crate::scene::Setup;
use crate::size::Size;

const HULLS: usize = 22;

fn create(size: Size, scale: u32) -> Result<Setup> {
    let (bodies, columns, rows) = size.pick((50, 150, 200), (5, 30, 40));
    let mut blueprint = Blueprint::new(true);
    blueprint.sleep_below(0.2);
    blueprint.add(BodyDef::fixed().shape(forms::wave(
        columns * scale,
        rows * scale,
        1.0 / scale as f32,
        0.4,
        0.05,
        0.1,
    )));
    let mut hulls = Vec::with_capacity(HULLS);
    let (mut y, mut r, l) = (1.0f32, 0.75f32, 1.5f32);
    for _ in 0..HULLS {
        hulls.push(
            Shape::new(forms::cylinder(l + 2.0 * r, r, y - r, 6)?)
                .friction(0.9)
                .rolling_resistance(0.05),
        );
        y += l + 2.0 * r;
        r *= 0.95;
    }
    let mut omega = -0.5f32;
    let mut z = -70.0f32;
    for index in 0..bodies {
        let position = Vec3::new(0.0, 1.0, z);
        let mut tree = BodyDef::dynamic().position(position);
        for hull in &hulls {
            tree = tree.shape(hull.clone());
        }
        let scale = 0.5 + 0.5 * index as f32 / bodies as f32;
        let spin = Vec3::new(0.0, 0.0, scale * omega);
        let lever = tree.center_of_mass() - position;
        blueprint.add(tree.velocity(spin.cross(lever), spin));
        z += 3.0;
        omega = -omega;
    }
    Ok(Setup::new(blueprint))
}

pub(super) fn create100(size: Size) -> Result<Setup> {
    create(size, 1)
}

pub(super) fn create50(size: Size) -> Result<Setup> {
    create(size, 2)
}

pub(super) fn create25(size: Size) -> Result<Setup> {
    create(size, 4)
}
