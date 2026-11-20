use std::ops::{Mul, Neg};

use glam::Quat;

use super::{MAX_LANES, Vec3W, Wide};

const BETWEEN_TOLERANCE: f32 = 100.0 * f32::EPSILON;

#[derive(Clone, Copy, Debug, Default)]
pub struct QuatW<W: Wide> {
    pub v: Vec3W<W>,
    pub s: W,
}

impl<W: Wide> QuatW<W> {
    #[inline(always)]
    pub fn new(v: Vec3W<W>, s: W) -> Self {
        Self { v, s }
    }

    #[inline(always)]
    pub fn identity() -> Self {
        Self::new(Vec3W::zero(), W::one())
    }

    #[inline(always)]
    pub fn splat(q: Quat) -> Self {
        Self::new(Vec3W::splat(glam::Vec3::new(q.x, q.y, q.z)), W::splat(q.w))
    }

    #[inline(always)]
    pub fn from_fn(mut f: impl FnMut(usize) -> Quat) -> Self {
        let mut values = [Quat::IDENTITY; MAX_LANES];
        for (lane, value) in values.iter_mut().enumerate().take(W::LANES) {
            *value = f(lane);
        }
        Self::new(
            Vec3W::from_fn(|lane| glam::Vec3::new(values[lane].x, values[lane].y, values[lane].z)),
            W::from_fn(|lane| values[lane].w),
        )
    }

    #[inline(always)]
    pub fn lane(&self, index: usize) -> Quat {
        let v = self.v.lane(index);
        Quat::from_xyzw(v.x, v.y, v.z, self.s.lane(index))
    }

    #[inline(always)]
    pub fn conjugate(self) -> Self {
        Self::new(-self.v, self.s)
    }

    #[inline(always)]
    pub fn dot(self, other: Self) -> W {
        self.s.mul_add(other.s, self.v.dot(other.v))
    }

    #[inline(always)]
    pub fn normalize(self) -> Self {
        let inverse = W::one() / self.dot(self).sqrt();
        Self::new(self.v.scale(inverse), self.s * inverse)
    }

    #[inline(always)]
    pub fn rotate(self, v: Vec3W<W>) -> Vec3W<W> {
        let t = self.v.cross(v);
        let t = t + t;
        v + t.scale(self.s) + self.v.cross(t)
    }

    #[inline(always)]
    pub fn rotate_inverse(self, v: Vec3W<W>) -> Vec3W<W> {
        self.conjugate().rotate(v)
    }

    #[inline(always)]
    pub fn integrate(self, angular_velocity: Vec3W<W>, h: W) -> Self {
        let half = W::splat(0.5) * h;
        let w = angular_velocity.scale(half);
        let delta = Self::new(w, W::default()) * self;
        Self::new(self.v + delta.v, self.s + delta.s).normalize()
    }

    #[inline(always)]
    pub fn select(mask: W::Mask, if_true: Self, if_false: Self) -> Self {
        Self::new(
            Vec3W::select(mask, if_true.v, if_false.v),
            W::select(mask, if_true.s, if_false.s),
        )
    }

    #[inline(always)]
    pub fn nlerp(self, other: Self, t: W) -> Self {
        let flip = self.dot(other).simd_lt(W::default());
        let other = Self::select(flip, -other, other);
        let s = W::one() - t;
        Self::new(self.v.scale(s) + other.v.scale(t), self.s * s + other.s * t).normalize()
    }

    #[inline(always)]
    pub fn from_rotation_arc(from: Vec3W<W>, to: Vec3W<W>) -> Self {
        let middle = (to - from).mul_add(W::splat(0.5), from);
        let regular = Self::new(from.cross(middle), from.dot(middle));
        let fallback = Self::new(from.any_orthogonal(), W::default());
        let open = middle
            .length_squared()
            .simd_gt(W::splat(BETWEEN_TOLERANCE * BETWEEN_TOLERANCE));
        Self::select(open, regular, fallback).normalize()
    }
}

impl<W: Wide> Neg for QuatW<W> {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self {
        Self::new(-self.v, -self.s)
    }
}

impl<W: Wide> Mul for QuatW<W> {
    type Output = Self;
    #[inline(always)]
    fn mul(self, other: Self) -> Self {
        let v = other.v.scale(self.s) + self.v.scale(other.s) + self.v.cross(other.v);
        let s = self.s * other.s - self.v.dot(other.v);
        Self::new(v, s)
    }
}
