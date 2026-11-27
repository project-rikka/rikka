use animate_body::HumanPose;
use animate_motion::CameraMotion;
use rikka_model::glam::{Quat, Vec3};
use rikka_model::mmd::FRAMES_PER_SECOND;

const SAMPLE_STEP: u32 = 3;
const MOST_CANDIDATES: u32 = 96;
const BACK_FACING_SHARE: f32 = 0.2;

fn opening_frames() -> u32 {
    FRAMES_PER_SECOND as u32
}

pub fn expressive_frame(
    last: u32,
    rest: &HumanPose,
    mut pose_at: impl FnMut(u32) -> Option<HumanPose>,
) -> u32 {
    let first = match last > 2 * opening_frames() {
        true => opening_frames(),
        false => 0,
    };
    let step = SAMPLE_STEP.max((last - first) / MOST_CANDIDATES);
    let mut best: Option<(u32, f32)> = None;
    for frame in (first..=last).step_by(step as usize) {
        let Some(pose) = pose_at(frame) else {
            continue;
        };
        let score = expressiveness(&pose, rest);
        if best.is_none_or(|(_, known)| score > known) {
            best = Some((frame, score));
        }
    }
    best.map_or(last / 2, |(frame, _)| frame)
}

fn expressiveness(pose: &HumanPose, rest: &HumanPose) -> f32 {
    let bent: f32 = pose
        .muscles
        .iter()
        .zip(&rest.muscles)
        .map(|(muscle, resting)| (muscle - resting).abs())
        .sum();
    bent * facing(pose.root_rot, rest.root_rot)
}

fn facing(root: Quat, rest: Quat) -> f32 {
    let heading = |rotation: Quat| (rotation * Vec3::Z).with_y(0.0).normalize_or(Vec3::Z);
    let toward = heading(root).dot(heading(rest));
    BACK_FACING_SHARE + (1.0 - BACK_FACING_SHARE) * (1.0 + toward) * 0.5
}

pub fn camera_frame(camera: &CameraMotion) -> u32 {
    let keys = camera.keys();
    let Some(first) = keys.first() else {
        return 0;
    };
    let mut longest = (first.frame, first.frame);
    let mut start = first.frame;
    for pair in keys.windows(2) {
        let (before, after) = (pair[0].frame, pair[1].frame);
        if after - before <= 1 {
            longest = longer(longest, (start, before));
            start = after;
        }
    }
    let last = keys.last().map_or(start, |key| key.frame);
    let (from, to) = longer(longest, (start, last));
    from + (to - from) / 2
}

fn longer(one: (u32, u32), two: (u32, u32)) -> (u32, u32) {
    match two.1 - two.0 > one.1 - one.0 {
        true => two,
        false => one,
    }
}
