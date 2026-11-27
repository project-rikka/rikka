use std::f32::consts::{FRAC_PI_2, PI, TAU};

use rikka_model_core::glam::Vec3;

const SECTORS: u32 = 24;
const CAP_RINGS: u32 = 6;
const SPHERE_RINGS: u32 = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Finish {
    Shell,
    Joint,
}

#[derive(Debug, Default)]
pub(crate) struct Part {
    pub(crate) positions: Vec<Vec3>,
    pub(crate) normals: Vec<Vec3>,
    pub(crate) joints: Vec<u16>,
    pub(crate) indices: Vec<u32>,
}

#[derive(Debug, Default)]
pub(crate) struct Shell {
    pub(crate) shell: Part,
    pub(crate) joint: Part,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Capsule {
    pub(crate) from: Vec3,
    pub(crate) to: Vec3,
    pub(crate) radius: f32,
    pub(crate) end_radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ellipsoid {
    pub(crate) center: Vec3,
    pub(crate) axes: Vec3,
}

struct Ring {
    center: Vec3,
    radius: f32,
    normal_along: f32,
    normal_out: f32,
}

impl Shell {
    fn part(&mut self, finish: Finish) -> &mut Part {
        match finish {
            Finish::Shell => &mut self.shell,
            Finish::Joint => &mut self.joint,
        }
    }

    pub(crate) fn capsule(&mut self, capsule: Capsule, joint: u16, finish: Finish) {
        let axis = (capsule.to - capsule.from).normalize_or(Vec3::Y);
        let mut rings = Vec::with_capacity(2 * CAP_RINGS as usize + 2);
        for (center, radius, start) in [
            (capsule.from, capsule.radius, -FRAC_PI_2),
            (capsule.to, capsule.end_radius, 0.0),
        ] {
            for step in 0..=CAP_RINGS {
                let angle = start + FRAC_PI_2 * step as f32 / CAP_RINGS as f32;
                let (sine, cosine) = angle.sin_cos();
                rings.push(Ring {
                    center: center + axis * (radius * sine),
                    radius: radius * cosine,
                    normal_along: sine,
                    normal_out: cosine,
                });
            }
        }
        self.part(finish).lathe(axis, &rings, joint);
    }

    pub(crate) fn ball(&mut self, center: Vec3, radius: f32, joint: u16) {
        self.ellipsoid(
            Ellipsoid {
                center,
                axes: Vec3::splat(radius),
            },
            joint,
            Finish::Joint,
        );
    }

    pub(crate) fn ellipsoid(&mut self, ellipsoid: Ellipsoid, joint: u16, finish: Finish) {
        let part = self.part(finish);
        let first = part.positions.len() as u32;
        for ring in 0..=SPHERE_RINGS {
            let latitude = -FRAC_PI_2 + PI * ring as f32 / SPHERE_RINGS as f32;
            let (sine, cosine) = latitude.sin_cos();
            for sector in 0..=SECTORS {
                let longitude = TAU * sector as f32 / SECTORS as f32;
                let unit = Vec3::new(cosine * longitude.cos(), sine, -cosine * longitude.sin());
                part.positions
                    .push(ellipsoid.center + unit * ellipsoid.axes);
                part.normals
                    .push((unit / ellipsoid.axes).normalize_or(Vec3::Y));
                part.joints.push(joint);
            }
        }
        part.grid(first, SPHERE_RINGS);
    }
}

impl Part {
    fn lathe(&mut self, axis: Vec3, rings: &[Ring], joint: u16) {
        let (u, v) = axis.any_orthonormal_pair();
        let first = self.positions.len() as u32;
        for ring in rings {
            for sector in 0..=SECTORS {
                let angle = TAU * sector as f32 / SECTORS as f32;
                let out = u * angle.cos() + v * angle.sin();
                self.positions.push(ring.center + out * ring.radius);
                self.normals
                    .push((axis * ring.normal_along + out * ring.normal_out).normalize_or(axis));
                self.joints.push(joint);
            }
        }
        self.grid(first, rings.len() as u32 - 1);
    }

    fn grid(&mut self, first: u32, rows: u32) {
        let stride = SECTORS + 1;
        for row in 0..rows {
            for sector in 0..SECTORS {
                let here = first + row * stride + sector;
                let beside = here + 1;
                let above = here + stride;
                let over = above + 1;
                self.indices
                    .extend_from_slice(&[here, beside, above, beside, over, above]);
            }
        }
    }
}
