use std::sync::LazyLock;

use rikka_model::glam::{Quat, Vec3};
use rikka_model::mmd::MMD_UNIT_SCALE;
use rikka_model::{Conversion, CoordinateSystem};

static INTO: LazyLock<Conversion> = LazyLock::new(|| {
    Conversion::new(
        CoordinateSystem::LEFT_HANDED_Y_UP,
        CoordinateSystem::CANONICAL,
        MMD_UNIT_SCALE,
    )
});

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MmdAxis {
    X,
    Y,
    Z,
}

impl MmdAxis {
    fn unit(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }

    pub fn turn(self, radians: f32) -> Quat {
        INTO.rotation(Quat::from_axis_angle(self.unit(), radians))
    }

    pub fn slide(self, units: f32) -> Vec3 {
        INTO.point(self.unit() * units)
    }

    pub fn letter(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }
}
