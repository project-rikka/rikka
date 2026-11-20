use glam::{Mat4, Quat, Vec3};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Pose {
    pub(crate) translation: Vec3,
    rotation: Quat,
}

impl Pose {
    pub(crate) fn from_matrix(matrix: Mat4) -> Pose {
        let (_, rotation, translation) = matrix.to_scale_rotation_translation();
        Pose {
            translation,
            rotation: rotation.normalize(),
        }
    }

    pub(crate) fn inverse(self) -> Pose {
        let rotation = self.rotation.inverse();
        Pose {
            translation: -(rotation * self.translation),
            rotation,
        }
    }

    pub(crate) fn apply(self, point: Vec3) -> Vec3 {
        self.rotation * point + self.translation
    }

    pub(crate) fn rotate(self, vector: Vec3) -> Vec3 {
        self.rotation * vector
    }
}
