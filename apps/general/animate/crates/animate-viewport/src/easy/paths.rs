use animate_body::Body;
use animate_motion::{HumanMotion, MotionBake, Part};
use rikka_model::glam::Vec3;

use crate::easy::part::end_of;
use crate::easy::style::PATH_REACH;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Path {
    pub(super) points: Vec<Vec3>,
    pub(super) keys: Vec<Vec3>,
}

pub(super) fn path(
    body: &Body,
    human: &HumanMotion,
    motion_bake: &MotionBake,
    part: Part,
    frame: u32,
) -> Option<Path> {
    let bone = end_of(part)?;
    let last = motion_bake.frames().checked_sub(1)?;
    let keyed: Vec<u32> = human
        .parts()
        .keys(&part)
        .iter()
        .map(|key| key.frame)
        .collect();
    let before = keyed.iter().copied().filter(|at| *at < frame).max();
    let after = keyed.iter().copied().filter(|at| *at > frame).min();
    let from = before
        .unwrap_or(0)
        .max(frame.saturating_sub(PATH_REACH))
        .min(last);
    let to = after
        .unwrap_or(u32::MAX)
        .min(frame.saturating_add(PATH_REACH))
        .min(last);
    if to <= from {
        return None;
    }
    let points: Vec<Vec3> = (from..=to)
        .map_while(|at| motion_bake.baked_pose_at(at))
        .map(|pose| body.fk(pose).position(bone))
        .collect();
    if points.len() < 2 {
        return None;
    }
    let drawn = from..from + points.len() as u32;
    let keys = keyed
        .into_iter()
        .filter(|at| drawn.contains(at))
        .map(|at| points[(at - from) as usize])
        .collect();
    Some(Path { points, keys })
}
