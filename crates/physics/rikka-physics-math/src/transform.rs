use glam::{Quat, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub p: Vec3,
    pub q: Quat,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        p: Vec3::ZERO,
        q: Quat::IDENTITY,
    };

    #[inline]
    pub const fn new(p: Vec3, q: Quat) -> Self {
        Self { p, q }
    }

    #[inline]
    pub const fn from_translation(p: Vec3) -> Self {
        Self::new(p, Quat::IDENTITY)
    }

    #[inline]
    pub const fn from_rotation(q: Quat) -> Self {
        Self::new(Vec3::ZERO, q)
    }

    #[inline]
    pub fn apply(&self, point: Vec3) -> Vec3 {
        self.q * point + self.p
    }

    #[inline]
    pub fn apply_inverse(&self, point: Vec3) -> Vec3 {
        self.q.inverse() * (point - self.p)
    }

    #[inline]
    pub fn rotate(&self, vector: Vec3) -> Vec3 {
        self.q * vector
    }

    #[inline]
    pub fn rotate_inverse(&self, vector: Vec3) -> Vec3 {
        self.q.inverse() * vector
    }

    #[inline]
    pub fn mul(&self, other: &Self) -> Self {
        Self::new(self.q * other.p + self.p, self.q * other.q)
    }

    #[inline]
    pub fn inverse_mul(&self, other: &Self) -> Self {
        let inverse = self.q.inverse();
        Self::new(inverse * (other.p - self.p), inverse * other.q)
    }

    #[inline]
    pub fn inverse(&self) -> Self {
        let inverse = self.q.inverse();
        Self::new(-(inverse * self.p), inverse)
    }

    #[inline]
    pub fn lerp(&self, other: &Self, alpha: f32) -> Self {
        Self::new(
            self.p.lerp(other.p, alpha),
            crate::nlerp(self.q, other.q, alpha),
        )
    }

    #[inline]
    pub fn is_finite(&self) -> bool {
        self.p.is_finite() && self.q.is_finite()
    }
}

impl std::ops::Mul for Transform {
    type Output = Self;

    #[inline]
    fn mul(self, other: Self) -> Self {
        Transform::mul(&self, &other)
    }
}
