use rikka_physics_math::{Quat, Vec3};

pub(crate) const LOCK_LINEAR_X: u32 = 1 << 0;
pub(crate) const LOCK_LINEAR_Y: u32 = 1 << 1;
pub(crate) const LOCK_LINEAR_Z: u32 = 1 << 2;
pub(crate) const LOCK_ANGULAR_X: u32 = 1 << 3;
pub(crate) const LOCK_ANGULAR_Y: u32 = 1 << 4;
pub(crate) const LOCK_ANGULAR_Z: u32 = 1 << 5;
pub(crate) const LOCKS: u32 = LOCK_LINEAR_X
    | LOCK_LINEAR_Y
    | LOCK_LINEAR_Z
    | LOCK_ANGULAR_X
    | LOCK_ANGULAR_Y
    | LOCK_ANGULAR_Z;
pub(crate) const DYNAMIC: u32 = 1 << 6;
pub(crate) const SPEED_CAPPED: u32 = 1 << 7;
pub(crate) const FAST_ROTATION: u32 = 1 << 8;

#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BodyState {
    pub v: Vec3,
    pub pad_v: f32,
    pub w: Vec3,
    pub pad_w: f32,
    pub dp: Vec3,
    pub flags: u32,
    pub dq: Quat,
}

const _: () = {
    assert!(size_of::<BodyState>() == 64);
    assert!(std::mem::offset_of!(BodyState, w) == 16);
    assert!(std::mem::offset_of!(BodyState, dp) == 32);
    assert!(std::mem::offset_of!(BodyState, dq) == 48);
};

impl BodyState {
    pub(crate) const IDENTITY: Self = Self {
        v: Vec3::ZERO,
        pad_v: 0.0,
        w: Vec3::ZERO,
        pad_w: 0.0,
        dp: Vec3::ZERO,
        flags: 0,
        dq: Quat::IDENTITY,
    };

    pub(crate) fn new(v: Vec3, w: Vec3, flags: u32) -> Self {
        Self {
            v,
            w,
            flags,
            ..Self::IDENTITY
        }
    }
}

impl Default for BodyState {
    fn default() -> Self {
        Self::IDENTITY
    }
}
