use rikka_physics_math::{Mat3, Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassData {
    pub mass: f32,
    pub center: Vec3,
    pub inertia: Mat3,
}

impl Default for MassData {
    fn default() -> Self {
        Self::ZERO
    }
}

impl MassData {
    pub const ZERO: Self = Self {
        mass: 0.0,
        center: Vec3::ZERO,
        inertia: Mat3::ZERO,
    };

    pub fn inertia_about(&self, point: Vec3) -> Mat3 {
        let d = self.center - point;
        let shift = Mat3::from_diagonal(Vec3::splat(d.length_squared())) - outer(d, d);
        self.inertia + shift * self.mass
    }

    pub fn rotated(&self, rotation: Quat) -> Self {
        let r = Mat3::from_quat(rotation);
        Self {
            mass: self.mass,
            center: rotation * self.center,
            inertia: r * self.inertia * r.transpose(),
        }
    }

    pub fn combine(parts: impl IntoIterator<Item = MassData>) -> Self {
        let parts: Vec<MassData> = parts.into_iter().collect();
        let mass: f32 = parts.iter().map(|part| part.mass).sum();
        if mass <= 0.0 {
            return Self::ZERO;
        }
        let center = parts
            .iter()
            .fold(Vec3::ZERO, |sum, part| sum + part.center * part.mass)
            / mass;
        let inertia = parts
            .iter()
            .fold(Mat3::ZERO, |sum, part| sum + part.inertia_about(center));
        Self {
            mass,
            center,
            inertia,
        }
    }
}

fn outer(a: Vec3, b: Vec3) -> Mat3 {
    Mat3::from_cols(a * b.x, a * b.y, a * b.z)
}
