use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

use glam::Vec3;

use super::{MAX_LANES, Wide};

#[derive(Clone, Copy, Debug, Default)]
pub struct Vec3W<W: Wide> {
    pub x: W,
    pub y: W,
    pub z: W,
}

impl<W: Wide> Vec3W<W> {
    #[inline(always)]
    pub fn new(x: W, y: W, z: W) -> Self {
        Self { x, y, z }
    }

    #[inline(always)]
    pub fn zero() -> Self {
        Self::splat(Vec3::ZERO)
    }

    #[inline(always)]
    pub fn splat(v: Vec3) -> Self {
        Self::new(W::splat(v.x), W::splat(v.y), W::splat(v.z))
    }

    #[inline(always)]
    pub fn from_fn(mut f: impl FnMut(usize) -> Vec3) -> Self {
        let mut x = [0.0f32; MAX_LANES];
        let mut y = [0.0f32; MAX_LANES];
        let mut z = [0.0f32; MAX_LANES];
        for lane in 0..W::LANES {
            let v = f(lane);
            x[lane] = v.x;
            y[lane] = v.y;
            z[lane] = v.z;
        }
        Self::new(W::load(&x), W::load(&y), W::load(&z))
    }

    #[inline(always)]
    pub fn gather(values: &[Vec3], indices: &[u32]) -> Self {
        Self::from_fn(|lane| values[indices[lane] as usize])
    }

    #[inline(always)]
    pub fn lane(&self, index: usize) -> Vec3 {
        Vec3::new(self.x.lane(index), self.y.lane(index), self.z.lane(index))
    }

    #[inline(always)]
    pub fn dot(self, other: Self) -> W {
        self.z
            .mul_add(other.z, self.y.mul_add(other.y, self.x * other.x))
    }

    #[inline(always)]
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    #[inline(always)]
    pub fn length_squared(self) -> W {
        self.dot(self)
    }

    #[inline(always)]
    pub fn length(self) -> W {
        self.length_squared().sqrt()
    }

    #[inline(always)]
    pub fn scale(self, s: W) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }

    #[inline(always)]
    pub fn mul_add(self, s: W, addend: Self) -> Self {
        Self::new(
            self.x.mul_add(s, addend.x),
            self.y.mul_add(s, addend.y),
            self.z.mul_add(s, addend.z),
        )
    }

    #[inline(always)]
    pub fn component_mul(self, other: Self) -> Self {
        Self::new(self.x * other.x, self.y * other.y, self.z * other.z)
    }

    #[inline(always)]
    pub fn normalize_or_zero(self) -> Self {
        let length = self.length();
        let valid = length.simd_gt(W::splat(f32::EPSILON));
        let inverse = W::select(valid, W::one() / length, W::default());
        self.scale(inverse)
    }

    #[inline(always)]
    pub fn select(mask: W::Mask, if_true: Self, if_false: Self) -> Self {
        Self::new(
            W::select(mask, if_true.x, if_false.x),
            W::select(mask, if_true.y, if_false.y),
            W::select(mask, if_true.z, if_false.z),
        )
    }

    #[inline(always)]
    pub fn any_orthogonal(self) -> Self {
        let across = self.x.abs().simd_gt(W::splat(0.5));
        Self::new(
            W::select(across, self.y, W::default()),
            W::select(across, -self.x, self.z),
            W::select(across, W::default(), -self.y),
        )
    }
}

impl<W: Wide> Add for Vec3W<W> {
    type Output = Self;
    #[inline(always)]
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl<W: Wide> Sub for Vec3W<W> {
    type Output = Self;
    #[inline(always)]
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl<W: Wide> AddAssign for Vec3W<W> {
    #[inline(always)]
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl<W: Wide> SubAssign for Vec3W<W> {
    #[inline(always)]
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl<W: Wide> Neg for Vec3W<W> {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl<W: Wide> Mul<W> for Vec3W<W> {
    type Output = Self;
    #[inline(always)]
    fn mul(self, s: W) -> Self {
        self.scale(s)
    }
}
