use rikka_physics_math::{Mat3, Vec3, Vec3W, Wide};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Sym3 {
    pub xx: f32,
    pub xy: f32,
    pub xz: f32,
    pub yy: f32,
    pub yz: f32,
    pub zz: f32,
}

impl Sym3 {
    pub(crate) const ZERO: Self = Self {
        xx: 0.0,
        xy: 0.0,
        xz: 0.0,
        yy: 0.0,
        yz: 0.0,
        zz: 0.0,
    };

    pub(crate) fn from_mat3(m: Mat3) -> Self {
        Self {
            xx: m.x_axis.x,
            xy: 0.5 * (m.y_axis.x + m.x_axis.y),
            xz: 0.5 * (m.z_axis.x + m.x_axis.z),
            yy: m.y_axis.y,
            yz: 0.5 * (m.z_axis.y + m.y_axis.z),
            zz: m.z_axis.z,
        }
    }

    pub(crate) fn to_mat3(self) -> Mat3 {
        Mat3::from_cols(
            Vec3::new(self.xx, self.xy, self.xz),
            Vec3::new(self.xy, self.yy, self.yz),
            Vec3::new(self.xz, self.yz, self.zz),
        )
    }

    #[inline]
    pub(crate) fn mul(&self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.xx * v.x + self.xy * v.y + self.xz * v.z,
            self.xy * v.x + self.yy * v.y + self.yz * v.z,
            self.xz * v.x + self.yz * v.y + self.zz * v.z,
        )
    }
    pub(crate) fn rotated(&self, rotation: Mat3) -> Self {
        Self::from_mat3(rotation * self.to_mat3() * rotation.transpose())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SymW<W: Wide> {
    pub xx: W,
    pub xy: W,
    pub xz: W,
    pub yy: W,
    pub yz: W,
    pub zz: W,
}

impl<W: Wide> SymW<W> {
    #[inline(always)]
    pub(crate) fn zero() -> Self {
        let z = W::default();
        Self {
            xx: z,
            xy: z,
            xz: z,
            yy: z,
            yz: z,
            zz: z,
        }
    }
    #[inline(always)]
    pub(crate) fn mul(&self, v: Vec3W<W>) -> Vec3W<W> {
        Vec3W::new(
            self.xz.mul_add(v.z, self.xy.mul_add(v.y, self.xx * v.x)),
            self.yz.mul_add(v.z, self.yy.mul_add(v.y, self.xy * v.x)),
            self.zz.mul_add(v.z, self.yz.mul_add(v.y, self.xz * v.x)),
        )
    }

    #[inline(always)]
    pub(crate) fn add(&self, other: &Self) -> Self {
        Self {
            xx: self.xx + other.xx,
            xy: self.xy + other.xy,
            xz: self.xz + other.xz,
            yy: self.yy + other.yy,
            yz: self.yz + other.yz,
            zz: self.zz + other.zz,
        }
    }

    #[inline(always)]
    pub(crate) fn inverse(&self) -> Self {
        let cxx = self.yy * self.zz - self.yz * self.yz;
        let cxy = self.xz * self.yz - self.xy * self.zz;
        let cxz = self.xy * self.yz - self.xz * self.yy;
        let cyy = self.xx * self.zz - self.xz * self.xz;
        let cyz = self.xy * self.xz - self.xx * self.yz;
        let czz = self.xx * self.yy - self.xy * self.xy;
        let det = self.xx * cxx + self.xy * cxy + self.xz * cxz;
        let valid = det.abs().simd_gt(W::splat(1000.0 * f32::MIN_POSITIVE));
        let inv = W::select(valid, W::one() / det, W::default());
        Self {
            xx: cxx * inv,
            xy: cxy * inv,
            xz: cxz * inv,
            yy: cyy * inv,
            yz: cyz * inv,
            zz: czz * inv,
        }
    }
}
