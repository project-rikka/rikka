use glam::{Mat3, Mat4, Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    #[must_use]
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    #[must_use]
    pub fn from_matrix(matrix: Mat4) -> Self {
        let axes = [
            matrix.x_axis.truncate(),
            matrix.y_axis.truncate(),
            matrix.z_axis.truncate(),
        ];
        let mut scale = Vec3::from_array(axes.map(Vec3::length));
        if Mat3::from_cols(axes[0], axes[1], axes[2]).determinant() < 0.0 {
            scale.x = -scale.x;
        }
        let mut units: [Option<Vec3>; 3] = std::array::from_fn(|axis| {
            let length = scale[axis];
            (length != 0.0 && length.is_finite()).then(|| axes[axis] / length)
        });
        for missing in 0..3 {
            let (next, last) = ((missing + 1) % 3, (missing + 2) % 3);
            if let (None, Some(a), Some(b)) = (units[missing], units[next], units[last]) {
                units[missing] = a.cross(b).try_normalize();
                if units[missing].is_none() {
                    units[last] = None;
                }
            }
        }
        let lone = {
            let mut known = (0..3).filter_map(|axis| Some((axis, units[axis]?)));
            known.next().filter(|_| known.next().is_none())
        };
        if let Some((axis, unit)) = lone {
            let (next, last) = unit.any_orthonormal_pair();
            units[(axis + 1) % 3] = Some(next);
            units[(axis + 2) % 3] = Some(last);
        }
        let rotation = match units {
            [Some(x), Some(y), Some(z)] => Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize(),
            _ => Quat::IDENTITY,
        };
        Self {
            translation: matrix.w_axis.truncate(),
            rotation: match rotation.is_finite() {
                true => rotation,
                false => Quat::IDENTITY,
            },
            scale,
        }
    }

    #[must_use]
    pub fn to_matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    #[must_use]
    pub fn lerp(self, other: Self, factor: f32) -> Self {
        Self {
            translation: self.translation.lerp(other.translation, factor),
            rotation: self.rotation.slerp(other.rotation, factor).normalize(),
            scale: self.scale.lerp(other.scale, factor),
        }
    }
}
