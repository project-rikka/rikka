use rikka_model_core::EulerOrder;
use rikka_model_core::glam::{Mat3, Mat4, Quat, Vec3, Vec4};

use crate::file::Record;

pub(crate) const ROTATION_QUATERNION: i64 = 0;
pub(crate) const ROTATION_AXIS_ANGLE: i64 = -1;

pub(crate) fn euler(angles: Vec3, mode: i64) -> Quat {
    let order = match mode {
        2 => EulerOrder::Xzy,
        3 => EulerOrder::Yxz,
        4 => EulerOrder::Yzx,
        5 => EulerOrder::Zxy,
        6 => EulerOrder::Zyx,
        _ => EulerOrder::Xyz,
    };
    order.quaternion(angles)
}

pub(crate) fn quaternion(wxyz: [f32; 4]) -> Quat {
    let [w, x, y, z] = wxyz;
    let quat = Quat::from_xyzw(x, y, z, w);
    match quat.length() {
        length if length > 1.0e-12 => quat / length,
        _ => Quat::IDENTITY,
    }
}

pub(crate) fn axis_angle(axis: Vec3, angle: f32) -> Quat {
    match axis.try_normalize() {
        Some(axis) => Quat::from_axis_angle(axis, angle),
        None => Quat::IDENTITY,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Rotation {
    Quaternion([f32; 4]),
    AxisAngle { axis: Vec3, angle: f32 },
    Euler { angles: Vec3, mode: i64 },
}

impl Rotation {
    pub(crate) const IDENTITY: Self = Self::Quaternion([1.0, 0.0, 0.0, 0.0]);

    pub(crate) fn read(record: &Record<'_>, mode: i64, delta: bool) -> Self {
        let field = |name: &'static str, delta_name: &'static str| match delta {
            true => delta_name,
            false => name,
        };
        match mode {
            ROTATION_QUATERNION => Self::Quaternion(
                record
                    .floats::<4>(field("quat", "dquat"))
                    .unwrap_or([1.0, 0.0, 0.0, 0.0]),
            ),
            ROTATION_AXIS_ANGLE => Self::AxisAngle {
                axis: record.vec3(field("rotAxis", "drotAxis")).unwrap_or(Vec3::Y),
                angle: record.float(field("rotAngle", "drotAngle")).unwrap_or(0.0),
            },
            mode => Self::Euler {
                angles: record
                    .vec3(field("rot", "drot"))
                    .or_else(|| record.vec3(field("eul", "deul")))
                    .unwrap_or(Vec3::ZERO),
                mode,
            },
        }
    }

    pub(crate) fn quat(self) -> Quat {
        match self {
            Self::Quaternion(wxyz) => quaternion(wxyz),
            Self::AxisAngle { axis, angle } => axis_angle(axis, angle),
            Self::Euler { angles, mode } => euler(angles, mode),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Resize {
    Times(Vec3),
    Plus(Vec3),
}

impl Resize {
    pub(crate) const NONE: Self = Self::Plus(Vec3::ZERO);

    pub(crate) fn from_object(object: &Record<'_>) -> Self {
        match object.vec3("dscale") {
            Some(delta) if object.blend().at_least(260, 6) => match delta == Vec3::ZERO {
                true => Self::NONE,
                false => Self::Times(delta),
            },
            _ => Self::Plus(object.vec3("dsize").unwrap_or(Vec3::ZERO)),
        }
    }

    pub(crate) fn apply(self, size: Vec3) -> Vec3 {
        match self {
            Self::Times(delta) => size * delta,
            Self::Plus(delta) => size + delta,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Basis {
    pub(crate) location: Vec3,
    pub(crate) rotation: Quat,
    pub(crate) scale: Vec3,
}

impl Basis {
    pub(crate) fn matrix(&self) -> Mat4 {
        Mat4::from_translation(self.location)
            * Mat4::from_mat3(Mat3::from_quat(self.rotation) * Mat3::from_diagonal(self.scale))
    }
}

pub(crate) fn object_basis(object: &Record<'_>) -> Basis {
    let mode = object.int("rotmode").unwrap_or(1);
    let location =
        object.vec3("loc").unwrap_or(Vec3::ZERO) + object.vec3("dloc").unwrap_or(Vec3::ZERO);
    let rotation =
        Rotation::read(object, mode, true).quat() * Rotation::read(object, mode, false).quat();
    Basis {
        location,
        rotation,
        scale: Resize::from_object(object).apply(stored_size(object)),
    }
}

pub(crate) fn stored_size(record: &Record<'_>) -> Vec3 {
    record
        .vec3("size")
        .or_else(|| record.vec3("scale"))
        .unwrap_or(Vec3::ONE)
}

pub(crate) fn finite(matrix: Mat4) -> Mat4 {
    match matrix.is_finite() {
        true => matrix,
        false => Mat4::IDENTITY,
    }
}

pub(crate) fn inverse(matrix: Mat4) -> Mat4 {
    match matrix.determinant().abs() > 1.0e-30 {
        true => matrix.inverse(),
        false => Mat4::IDENTITY,
    }
}

pub(crate) fn triangle_frame(a: Vec3, b: Vec3, c: Vec3) -> Mat4 {
    let normal = (b - a).cross(c - a).normalize_or_zero();
    let along = (b - a).normalize_or_zero();
    let side = normal.cross(along);
    let rotation = match normal == Vec3::ZERO || along == Vec3::ZERO {
        true => Mat3::IDENTITY,
        false => Mat3::from_cols(along, side, normal),
    };
    let mut matrix = Mat4::from_mat3(rotation);
    matrix.w_axis = Vec4::from(((a + b + c) / 3.0, 1.0));
    matrix
}
