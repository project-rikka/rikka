use rikka_model_core::EulerOrder;
use rikka_model_core::glam::{Quat, Vec3};

pub(crate) const DEGREES: f32 = std::f32::consts::PI / 180.0;

pub(crate) fn order_of(rotation_order: i64) -> EulerOrder {
    match rotation_order {
        1 => EulerOrder::Xzy,
        2 => EulerOrder::Yzx,
        3 => EulerOrder::Yxz,
        4 => EulerOrder::Zxy,
        5 => EulerOrder::Zyx,
        _ => EulerOrder::Xyz,
    }
}

pub(crate) fn quaternion(order: EulerOrder, degrees: Vec3) -> Quat {
    order.quaternion(degrees * DEGREES)
}

pub(crate) fn angles(order: EulerOrder, rotation: Quat) -> Vec3 {
    order.angles(rotation) / DEGREES
}

pub(crate) fn camera_facing() -> Quat {
    Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
}

pub(crate) fn light_facing() -> Quat {
    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}
