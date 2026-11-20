use glam::{EulerRot, Quat, Vec3};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EulerOrder {
    #[default]
    Xyz,
    Xzy,
    Yzx,
    Yxz,
    Zxy,
    Zyx,
}

impl EulerOrder {
    const fn composition(self) -> (EulerRot, [usize; 3]) {
        match self {
            Self::Xyz => (EulerRot::ZYX, [2, 1, 0]),
            Self::Xzy => (EulerRot::YZX, [1, 2, 0]),
            Self::Yzx => (EulerRot::XZY, [0, 2, 1]),
            Self::Yxz => (EulerRot::ZXY, [2, 0, 1]),
            Self::Zxy => (EulerRot::YXZ, [1, 0, 2]),
            Self::Zyx => (EulerRot::XYZ, [0, 1, 2]),
        }
    }

    #[must_use]
    pub fn quaternion(self, angles: Vec3) -> Quat {
        let (composition, [first, second, third]) = self.composition();
        Quat::from_euler(composition, angles[first], angles[second], angles[third])
    }

    #[must_use]
    pub fn angles(self, rotation: Quat) -> Vec3 {
        let (composition, [first, second, third]) = self.composition();
        let turned = rotation.to_euler(composition);
        let mut angles = Vec3::ZERO;
        angles[first] = turned.0;
        angles[second] = turned.1;
        angles[third] = turned.2;
        angles
    }
}
