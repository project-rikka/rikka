use rikka_physics_math::Transform;

use super::def::{BodyKind, MassSpec};

pub(crate) const NONE: u32 = u32::MAX;

pub(crate) const BULLET: u32 = 1 << 0;
pub(crate) const SLEEP_ENABLED: u32 = 1 << 1;
pub(crate) const CHARACTER: u32 = 1 << 2;
pub(crate) const FAST_ROTATION: u32 = 1 << 4;

#[derive(Clone, Copy, Debug)]
pub(crate) struct BodyRecord {
    pub kind: BodyKind,
    pub set: u32,
    pub local: u32,
    pub island: u32,
    pub island_prev: u32,
    pub island_next: u32,
    pub shape_head: u32,
    pub shape_count: u32,
    pub contact_head: u32,
    pub contact_count: u32,
    pub joint_head: u32,
    pub joint_count: u32,
    pub alive: bool,
    pub flags: u32,
    pub locks: u32,
    pub sleep_time: f32,
    pub sleep_threshold: f32,
    pub user_data: u64,
    pub mass: MassSpec,
    pub target: Option<Transform>,
    pub move_event: u32,
}
