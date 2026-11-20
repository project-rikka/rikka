use std::ops::{Range, RangeInclusive};

use animate_body::{
    Body, HEAD, HumanPose, LEFT_ARM, LEFT_FINGERS, LEFT_LEG, RIGHT_ARM, RIGHT_FINGERS, RIGHT_LEG,
    TORSO, rotation_vector,
};
use rikka_model_core::glam::{Quat, Vec3, Vec4};

use crate::sample::PoseSample;

const SLOW_FRAMES: f32 = 24.0;
const FAST_FRAMES: f32 = 2.0;
const PASSES: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intensity {
    pub size: f32,
    pub snap: f32,
    pub torso: f32,
    pub head: f32,
    pub arms: f32,
    pub legs: f32,
    pub bounce: f32,
}

impl Default for Intensity {
    fn default() -> Self {
        Self {
            size: 1.0,
            snap: 1.0,
            torso: 1.0,
            head: 1.0,
            arms: 1.0,
            legs: 1.0,
            bounce: 1.0,
        }
    }
}

struct Band {
    gain: f32,
    snap: f32,
}

impl Band {
    fn shaped(&self, raw: f32, fast: f32, slow: f32) -> f32 {
        slow + self.gain * ((fast - slow) + self.snap * (raw - fast))
    }
}

impl Intensity {
    pub const RANGE: RangeInclusive<f32> = 0.0..=2.0;

    #[must_use]
    pub fn is_neutral(&self) -> bool {
        *self == Self::default()
    }

    pub(crate) fn reach(&self) -> u32 {
        match self.is_neutral() {
            true => 0,
            false => (PASSES * box_radius(SLOW_FRAMES)) as u32 + 1,
        }
    }

    pub(crate) fn shape(&self, body: &Body, frames: &mut [PoseSample], limits: bool) {
        if self.is_neutral() || frames.is_empty() {
            return;
        }
        let band = |gain: f32| Band {
            gain: self.size * gain,
            snap: self.snap,
        };
        let originals: Vec<HumanPose> = frames.iter().map(|frame| frame.pose).collect();
        let regions: [(Range<usize>, f32); 8] = [
            (TORSO, self.torso),
            (HEAD, self.head),
            (LEFT_ARM, self.arms),
            (RIGHT_ARM, self.arms),
            (LEFT_FINGERS, self.arms),
            (RIGHT_FINGERS, self.arms),
            (LEFT_LEG, self.legs),
            (RIGHT_LEG, self.legs),
        ];
        for (range, gain) in regions {
            let band = band(gain);
            for muscle in range {
                let raw: Vec<f32> = originals.iter().map(|pose| pose.muscles[muscle]).collect();
                let (slow, fast) = (smooth(&raw, SLOW_FRAMES), smooth(&raw, FAST_FRAMES));
                for (at, frame) in frames.iter_mut().enumerate() {
                    frame.pose.muscles[muscle] = band.shaped(raw[at], fast[at], slow[at]);
                }
            }
        }
        let sway = band(self.torso);
        let bounce = band(self.bounce);
        for axis in 0..3 {
            let raw: Vec<f32> = originals.iter().map(|pose| pose.root_pos[axis]).collect();
            let (slow, fast) = (smooth(&raw, SLOW_FRAMES), smooth(&raw, FAST_FRAMES));
            let band = match axis {
                1 => &bounce,
                _ => &sway,
            };
            for (at, frame) in frames.iter_mut().enumerate() {
                frame.pose.root_pos[axis] = band.shaped(raw[at], fast[at], slow[at]);
            }
        }
        self.turn(&sway, &originals, frames);
        for (frame, original) in frames.iter_mut().zip(&originals) {
            body.clamp_around(&mut frame.pose, original, limits);
        }
    }

    fn turn(&self, band: &Band, originals: &[HumanPose], frames: &mut [PoseSample]) {
        let mut aligned: Vec<Quat> = Vec::with_capacity(originals.len());
        for pose in originals {
            let turn = pose.root_rot.normalize();
            let turn = match aligned.last() {
                Some(before) if before.dot(turn) < 0.0 => -turn,
                _ => turn,
            };
            aligned.push(turn);
        }
        let mut slow: Vec<Vec4> = vec![Vec4::ZERO; aligned.len()];
        for lane in 0..4 {
            let raw: Vec<f32> = aligned.iter().map(|turn| Vec4::from(*turn)[lane]).collect();
            for (row, value) in slow.iter_mut().zip(smooth(&raw, SLOW_FRAMES)) {
                row[lane] = value;
            }
        }
        let slow: Vec<Quat> = slow
            .into_iter()
            .zip(&aligned)
            .map(|(offset, turn)| match offset.length() > f32::EPSILON {
                true => Quat::from_vec4(offset.normalize()),
                false => *turn,
            })
            .collect();
        let offsets: Vec<Vec3> = slow
            .iter()
            .zip(&aligned)
            .map(|(slow, turn)| rotation_vector(slow.inverse() * *turn))
            .collect();
        let mut fast = vec![Vec3::ZERO; offsets.len()];
        for lane in 0..3 {
            let raw: Vec<f32> = offsets.iter().map(|offset| offset[lane]).collect();
            for (row, value) in fast.iter_mut().zip(smooth(&raw, FAST_FRAMES)) {
                row[lane] = value;
            }
        }
        for (at, frame) in frames.iter_mut().enumerate() {
            let shaped = (fast[at] + band.snap * (offsets[at] - fast[at])) * band.gain;
            frame.pose.root_rot = (slow[at] * Quat::from_scaled_axis(shaped)).normalize();
        }
    }
}

fn box_radius(sigma: f32) -> usize {
    let width = (12.0 * sigma * sigma / PASSES as f32 + 1.0).sqrt();
    ((width - 1.0) * 0.5).round().max(0.0) as usize
}

fn smooth(values: &[f32], sigma: f32) -> Vec<f32> {
    let radius = box_radius(sigma);
    let mut smoothed = values.to_vec();
    if radius == 0 || values.len() < 2 {
        return smoothed;
    }
    let last = values.len() as isize - 1;
    let width = (2 * radius + 1) as f32;
    for _ in 0..PASSES {
        let source = smoothed.clone();
        let at = |index: isize| source[index.clamp(0, last) as usize];
        let mut sum: f32 = (-(radius as isize)..=radius as isize).map(at).sum();
        for (index, out) in smoothed.iter_mut().enumerate() {
            *out = sum / width;
            let index = index as isize;
            sum += at(index + radius as isize + 1) - at(index - radius as isize);
        }
    }
    smoothed
}
