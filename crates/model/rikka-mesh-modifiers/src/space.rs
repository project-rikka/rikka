use glam::{Mat3, Mat4, Vec3};

const SINGULAR: f32 = 1.0e-30;

pub(crate) fn inverse(matrix: Mat4) -> Mat4 {
    match matrix.determinant().abs() > SINGULAR {
        true => matrix.inverse(),
        false => Mat4::IDENTITY,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpaceTransform {
    local_to_target: Mat4,
    target_to_local: Mat4,
}

impl SpaceTransform {
    pub(crate) fn new(local: Mat4, target: Mat4) -> SpaceTransform {
        let local_to_target = inverse(target) * local;
        SpaceTransform {
            local_to_target,
            target_to_local: inverse(local_to_target),
        }
    }

    pub(crate) fn apply(&self, point: Vec3) -> Vec3 {
        self.local_to_target.transform_point3(point)
    }

    pub(crate) fn invert(&self, point: Vec3) -> Vec3 {
        self.target_to_local.transform_point3(point)
    }

    pub(crate) fn apply_normal(&self, normal: Vec3) -> Vec3 {
        (Mat3::from_mat4(self.local_to_target) * normal).normalize_or_zero()
    }

    pub(crate) fn invert_normal(&self, normal: Vec3) -> Vec3 {
        (Mat3::from_mat4(self.target_to_local) * normal).normalize_or_zero()
    }
}
