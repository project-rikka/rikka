use std::f64::consts::PI;

use rikka_physics_math::{Mat3, Vec3, quat_between};

use crate::MassData;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Revolution {
    volume: f64,
    first: f64,
    second: f64,
    quartic: f64,
}

fn integrate(coefficients: &[f64], z0: f64, z1: f64) -> f64 {
    coefficients
        .iter()
        .enumerate()
        .map(|(power, c)| {
            let n = power as i32 + 1;
            c * (z1.powi(n) - z0.powi(n)) / f64::from(n)
        })
        .sum()
}

fn multiply(a: &[f64; 3], b: &[f64; 3]) -> [f64; 5] {
    let mut out = [0.0; 5];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            out[i + j] += x * y;
        }
    }
    out
}

impl Revolution {
    fn add(&mut self, square: [f64; 3], z0: f64, z1: f64) {
        if z1 <= z0 {
            return;
        }
        self.volume += PI * integrate(&square, z0, z1);
        self.first += PI * integrate(&[0.0, square[0], square[1], square[2]], z0, z1);
        self.second += PI * integrate(&[0.0, 0.0, square[0], square[1], square[2]], z0, z1);
        self.quartic += PI * integrate(&multiply(&square, &square), z0, z1);
    }

    pub(crate) fn ball_slice(&mut self, center: f64, radius: f64, z0: f64, z1: f64) {
        self.add(
            [radius * radius - center * center, 2.0 * center, -1.0],
            z0.max(center - radius),
            z1.min(center + radius),
        );
    }

    pub(crate) fn frustum(&mut self, z0: f64, z1: f64, radius0: f64, radius1: f64) {
        if z1 <= z0 {
            return;
        }
        let slope = (radius1 - radius0) / (z1 - z0);
        let intercept = radius0 - slope * z0;
        self.add(
            [
                intercept * intercept,
                2.0 * intercept * slope,
                slope * slope,
            ],
            z0,
            z1,
        );
    }

    pub(crate) fn mass(&self, density: f32, base: Vec3, axis: Vec3) -> MassData {
        let density = f64::from(density);
        let mass = density * self.volume;
        if self.volume <= 0.0 {
            return MassData::ZERO;
        }
        let center = self.first / self.volume;
        let axial = density * 0.5 * self.quartic;
        let transverse = density * (0.25 * self.quartic + self.second) - mass * center * center;
        let local = Mat3::from_diagonal(Vec3::new(
            transverse as f32,
            axial as f32,
            transverse as f32,
        ));
        let rotation = Mat3::from_quat(quat_between(Vec3::Y, axis));
        MassData {
            mass: mass as f32,
            center: base + axis * center as f32,
            inertia: rotation * local * rotation.transpose(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Frustum {
    pub(crate) base: Vec3,
    pub(crate) axis: Vec3,
    pub(crate) z0: f32,
    pub(crate) z1: f32,
    pub(crate) radius0: f32,
    pub(crate) radius1: f32,
}

impl Frustum {
    fn slope(&self) -> f32 {
        (self.radius1 - self.radius0) / (self.z1 - self.z0)
    }

    pub(crate) fn contains(&self, point: Vec3) -> bool {
        let offset = point - self.base;
        let z = offset.dot(self.axis);
        if z < self.z0 || z > self.z1 {
            return false;
        }
        let radius = self.radius0 + self.slope() * (z - self.z0);
        (offset - z * self.axis).length_squared() <= radius * radius
    }

    pub(crate) fn ray(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
    ) -> Option<(f32, Vec3)> {
        let k = self.slope();
        let offset = origin - self.base;
        let zo = offset.dot(self.axis);
        let zd = direction.dot(self.axis);
        let w = offset - zo * self.axis;
        let v = direction - zd * self.axis;
        let radius_at_origin = self.radius0 + k * (zo - self.z0);

        let a = v.dot(v) - k * k * zd * zd;
        let b = w.dot(v) - k * radius_at_origin * zd;
        let c = w.dot(w) - radius_at_origin * radius_at_origin;

        let mut roots = [f32::MAX; 2];
        if a.abs() > 1.0e-12 {
            let discriminant = b * b - a * c;
            if discriminant < 0.0 {
                return None;
            }
            let root = discriminant.sqrt();
            roots = [(-b - root) / a, (-b + root) / a];
            if roots[0] > roots[1] {
                roots.swap(0, 1);
            }
        } else if b.abs() > 1.0e-12 {
            roots[0] = -0.5 * c / b;
        }

        for t in roots {
            if !(0.0..=max_distance).contains(&t) {
                continue;
            }
            let z = zo + t * zd;
            if z < self.z0 || z > self.z1 || self.radius0 + k * (z - self.z0) < 0.0 {
                continue;
            }
            let radial = (w + t * v).normalize_or_zero();
            let normal = (radial - k * self.axis).normalize_or_zero();
            if normal.dot(direction) < 0.0 {
                return Some((t, normal));
            }
        }
        None
    }
}
