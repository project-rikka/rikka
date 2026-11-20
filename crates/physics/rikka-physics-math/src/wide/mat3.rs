use std::ops::Add;

use glam::Mat3;

use super::{MAX_LANES, QuatW, Vec3W, Wide};

#[derive(Clone, Copy, Debug, Default)]
pub struct Mat3W<W: Wide> {
    pub x_axis: Vec3W<W>,
    pub y_axis: Vec3W<W>,
    pub z_axis: Vec3W<W>,
}

impl<W: Wide> Mat3W<W> {
    #[inline(always)]
    pub fn from_cols(x_axis: Vec3W<W>, y_axis: Vec3W<W>, z_axis: Vec3W<W>) -> Self {
        Self {
            x_axis,
            y_axis,
            z_axis,
        }
    }

    #[inline(always)]
    pub fn zero() -> Self {
        Self::splat(Mat3::ZERO)
    }

    #[inline(always)]
    pub fn splat(m: Mat3) -> Self {
        Self::from_cols(
            Vec3W::splat(m.x_axis),
            Vec3W::splat(m.y_axis),
            Vec3W::splat(m.z_axis),
        )
    }

    #[inline(always)]
    pub fn from_fn(mut f: impl FnMut(usize) -> Mat3) -> Self {
        let mut values = [Mat3::ZERO; MAX_LANES];
        for (lane, value) in values.iter_mut().enumerate().take(W::LANES) {
            *value = f(lane);
        }
        Self::from_cols(
            Vec3W::from_fn(|lane| values[lane].x_axis),
            Vec3W::from_fn(|lane| values[lane].y_axis),
            Vec3W::from_fn(|lane| values[lane].z_axis),
        )
    }

    #[inline(always)]
    pub fn lane(&self, index: usize) -> Mat3 {
        Mat3::from_cols(
            self.x_axis.lane(index),
            self.y_axis.lane(index),
            self.z_axis.lane(index),
        )
    }

    #[inline(always)]
    pub fn from_quat(q: QuatW<W>) -> Self {
        Self::from_cols(
            q.rotate(Vec3W::splat(glam::Vec3::X)),
            q.rotate(Vec3W::splat(glam::Vec3::Y)),
            q.rotate(Vec3W::splat(glam::Vec3::Z)),
        )
    }

    #[inline(always)]
    pub fn mul_vec(&self, v: Vec3W<W>) -> Vec3W<W> {
        self.z_axis
            .mul_add(v.z, self.y_axis.mul_add(v.y, self.x_axis.scale(v.x)))
    }

    #[inline(always)]
    pub fn mul_mat(&self, other: &Self) -> Self {
        Self::from_cols(
            self.mul_vec(other.x_axis),
            self.mul_vec(other.y_axis),
            self.mul_vec(other.z_axis),
        )
    }

    #[inline(always)]
    pub fn transpose(&self) -> Self {
        Self::from_cols(
            Vec3W::new(self.x_axis.x, self.y_axis.x, self.z_axis.x),
            Vec3W::new(self.x_axis.y, self.y_axis.y, self.z_axis.y),
            Vec3W::new(self.x_axis.z, self.y_axis.z, self.z_axis.z),
        )
    }

    #[inline(always)]
    pub fn determinant(&self) -> W {
        self.z_axis.dot(self.x_axis.cross(self.y_axis))
    }

    #[inline(always)]
    pub fn inverse_or_zero(&self) -> Self {
        let tmp0 = self.y_axis.cross(self.z_axis);
        let tmp1 = self.z_axis.cross(self.x_axis);
        let tmp2 = self.x_axis.cross(self.y_axis);
        let det = self.z_axis.dot(tmp2);
        let valid = det.abs().simd_gt(W::splat(f32::MIN_POSITIVE));
        let inverse = W::select(valid, W::one() / det, W::default());
        Self::from_cols(
            tmp0.scale(inverse),
            tmp1.scale(inverse),
            tmp2.scale(inverse),
        )
        .transpose()
    }

    #[inline(always)]
    pub fn solve(&self, b: Vec3W<W>) -> Vec3W<W> {
        self.inverse_or_zero().mul_vec(b)
    }

    #[inline(always)]
    pub fn rotated(&self, q: QuatW<W>) -> Self {
        let rotation = Self::from_quat(q);
        rotation.mul_mat(self).mul_mat(&rotation.transpose())
    }
}

impl<W: Wide> Add for Mat3W<W> {
    type Output = Self;
    #[inline(always)]
    fn add(self, other: Self) -> Self {
        Self::from_cols(
            self.x_axis + other.x_axis,
            self.y_axis + other.y_axis,
            self.z_axis + other.z_axis,
        )
    }
}
