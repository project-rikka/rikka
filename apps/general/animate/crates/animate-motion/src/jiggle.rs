use std::f32::consts::TAU;

use animate_body::{Body, Lobe, Worlds, arc};
use rikka_model_core::glam::{Quat, Vec3};

use crate::tuning::FRAME_SECONDS;

const SUBSTEPS: usize = 4;
const STIFF_HZ: f32 = 9.0;
const SOFT_HZ: f32 = 3.5;
const FIRM_DAMPING: f32 = 0.6;
const LOOSE_DAMPING: f32 = 0.2;
const MIN_DEGREES: f32 = 8.0;
const MAX_DEGREES: f32 = 30.0;
const CLEARANCE: f32 = 0.5;
const REST_AMPLITUDE: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Tip {
    at: Vec3,
    speed: Vec3,
    root: Vec3,
    target: Vec3,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Jiggles {
    tips: Vec<Option<Tip>>,
}

struct Placed {
    root: Vec3,
    target: Vec3,
    parent: Quat,
}

fn placed(lobe: &Lobe, worlds: &Worlds) -> Placed {
    let joint = worlds.joint(lobe.anchor);
    let root = joint.point(lobe.root.position);
    let turn = joint.rotation * lobe.root.rotation;
    Placed {
        root,
        target: root + turn * lobe.tip,
        parent: joint.rotation * lobe.parent,
    }
}

struct Spring {
    pitch: f32,
    damping: f32,
    widest: f32,
}

impl Spring {
    fn from_strength(strength: f32) -> Option<Self> {
        let strength = strength.clamp(0.0, 1.0);
        (strength > 0.0).then(|| Self {
            pitch: TAU * (STIFF_HZ + (SOFT_HZ - STIFF_HZ) * strength),
            damping: FIRM_DAMPING + (LOOSE_DAMPING - FIRM_DAMPING) * strength,
            widest: (MIN_DEGREES + (MAX_DEGREES - MIN_DEGREES) * strength).to_radians(),
        })
    }
}

pub(crate) fn settling_frames(strength: f32) -> u32 {
    Spring::from_strength(strength).map_or(0, |spring| {
        let seconds = -REST_AMPLITUDE.ln() / (spring.damping * spring.pitch);
        (seconds / FRAME_SECONDS).ceil() as u32
    })
}

impl Jiggles {
    pub(crate) fn step(
        &mut self,
        body: &Body,
        worlds: &Worlds,
        strength: f32,
        obstacles: &[Vec3],
    ) -> Vec<Quat> {
        let lobes = body.lobes();
        let Some(Spring {
            pitch,
            damping,
            widest,
        }) = Spring::from_strength(strength)
        else {
            self.tips.clear();
            return Vec::new();
        };
        self.tips.resize(lobes.len(), None);
        let step = FRAME_SECONDS / SUBSTEPS as f32;
        lobes
            .iter()
            .zip(&mut self.tips)
            .map(|(lobe, tip_slot)| {
                let now = placed(lobe, worlds);
                let length = lobe.tip.length();
                let mut tip = tip_slot.unwrap_or(Tip {
                    at: now.target,
                    speed: Vec3::ZERO,
                    root: now.root,
                    target: now.target,
                });
                let carried = (now.target - tip.target) / FRAME_SECONDS;
                for substep in 1..=SUBSTEPS {
                    let share = substep as f32 / SUBSTEPS as f32;
                    let root = tip.root.lerp(now.root, share);
                    let target = tip.target.lerp(now.target, share);
                    let pull = (target - tip.at) * pitch * pitch
                        - (tip.speed - carried) * (2.0 * damping * pitch);
                    let before = tip.at;
                    let moved = tip.at + (tip.speed + pull * step) * step;
                    let constrained = constrain(
                        moved,
                        root,
                        target,
                        length,
                        widest,
                        obstacles,
                        length * CLEARANCE,
                    );
                    tip.speed = (constrained - before) / step;
                    tip.at = constrained;
                }
                tip.root = now.root;
                tip.target = now.target;
                *tip_slot = Some(tip);
                let swing = arc(now.target - now.root, tip.at - now.root);
                (now.parent.inverse() * swing * now.parent).normalize()
            })
            .collect()
    }
}

fn constrain(
    at: Vec3,
    root: Vec3,
    target: Vec3,
    length: f32,
    widest: f32,
    obstacles: &[Vec3],
    clearance: f32,
) -> Vec3 {
    let rest = (target - root).normalize_or(Vec3::Y);
    let mut position = at;
    for point in obstacles {
        let away = position - *point;
        let distance = away.length();
        if distance < clearance && distance > f32::EPSILON {
            position = *point + away * (clearance / distance);
        }
    }
    let toward = (position - root).normalize_or(rest);
    let angle = rest.angle_between(toward);
    let toward = match angle > widest {
        true => Quat::from_rotation_arc(rest, toward)
            .slerp(Quat::IDENTITY, 1.0 - widest / angle)
            .mul_vec3(rest),
        false => toward,
    };
    root + toward * length
}
