use glam::{Mat3, Mat4, Quat, Vec3, Vec4};

use crate::transform::Transform;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
    NegX,
    NegY,
    NegZ,
}

impl Axis {
    #[must_use]
    pub const fn direction(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
            Self::NegX => Vec3::NEG_X,
            Self::NegY => Vec3::NEG_Y,
            Self::NegZ => Vec3::NEG_Z,
        }
    }

    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::X => Self::NegX,
            Self::Y => Self::NegY,
            Self::Z => Self::NegZ,
            Self::NegX => Self::X,
            Self::NegY => Self::Y,
            Self::NegZ => Self::Z,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoordinateSystem {
    pub right: Axis,
    pub up: Axis,
    pub forward: Axis,
}

impl CoordinateSystem {
    pub const CANONICAL: Self = Self {
        right: Axis::X,
        up: Axis::Y,
        forward: Axis::NegZ,
    };

    pub const LEFT_HANDED_Y_UP: Self = Self {
        right: Axis::X,
        up: Axis::Y,
        forward: Axis::Z,
    };

    pub const RIGHT_HANDED_Z_UP: Self = Self {
        right: Axis::X,
        up: Axis::Z,
        forward: Axis::Y,
    };

    #[must_use]
    pub fn frame(self) -> Mat3 {
        Mat3::from_cols(
            self.right.direction(),
            self.up.direction(),
            self.forward.direction(),
        )
    }

    #[must_use]
    pub fn is_right_handed(self) -> bool {
        self.right
            .direction()
            .cross(self.up.direction())
            .dot(self.forward.direction())
            < 0.0
    }
}

impl Default for CoordinateSystem {
    fn default() -> Self {
        Self::CANONICAL
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conversion {
    basis: Mat3,
    scale: f32,
}

impl Conversion {
    #[must_use]
    pub fn new(from: CoordinateSystem, to: CoordinateSystem, scale: f32) -> Self {
        Self {
            basis: to.frame() * from.frame().transpose(),
            scale,
        }
    }

    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.basis.abs_diff_eq(Mat3::IDENTITY, 1e-6) && (self.scale - 1.0).abs() < 1e-6
    }

    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    #[must_use]
    pub fn flips_winding(&self) -> bool {
        self.basis.determinant() < 0.0
    }

    #[must_use]
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_mat3(self.basis) * Mat4::from_scale(Vec3::splat(self.scale))
    }

    #[must_use]
    pub fn point(&self, point: Vec3) -> Vec3 {
        self.basis * point * self.scale
    }

    #[must_use]
    pub fn vector(&self, vector: Vec3) -> Vec3 {
        self.basis * vector
    }

    #[must_use]
    pub fn rotation(&self, rotation: Quat) -> Quat {
        let length = rotation.length();
        if length < 1e-9 {
            return rotation;
        }
        let unit = Quat::from_vec4(Vec4::from(rotation) / length);
        let turned = self.basis * Mat3::from_quat(unit) * self.basis.transpose();
        Quat::from_vec4(Vec4::from(Quat::from_mat3(&turned).normalize()) * length)
    }

    #[must_use]
    pub fn transform(&self, transform: Transform) -> Transform {
        Transform {
            translation: self.point(transform.translation),
            rotation: self.rotation(transform.rotation),
            scale: self.scaling(transform.scale),
        }
    }

    #[must_use]
    pub fn scaling(&self, scaling: Vec3) -> Vec3 {
        Mat3::from_cols(
            self.basis.x_axis.abs(),
            self.basis.y_axis.abs(),
            self.basis.z_axis.abs(),
        ) * scaling
    }
}
