use std::collections::HashMap;
use std::collections::hash_map::Entry;

use animate_body::{Body, HumanPose};
use rikka_model_core::glam::Vec3;

use crate::physique::Physique;
use crate::scene::{Pair, Scene};

const BISECTIONS: usize = 16;
const NUDGE: f32 = 1e-3;
const SHORTEST_PATH: f32 = 1e-4;
const LONGEST_PATH: f32 = 0.25;
const FIRST_STRIDE: f32 = 0.01;
const COHERENT: f32 = 0.5;
pub(crate) const PATH_FRAMES: usize = 6;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Intrusion {
    pub(crate) probe: usize,
    pub(crate) obstacle: usize,
    pub(crate) exit: Vec3,
    pub(crate) toward: Vec3,
}

impl Intrusion {
    pub(crate) fn depth(&self, local: Vec3) -> f32 {
        self.toward.dot(self.exit - local)
    }
}

pub(crate) type Inside = Vec<(Pair, Vec3)>;

pub(crate) fn scan(physique: &Physique, body: &Body, pose: &HumanPose) -> Inside {
    let scene = Scene::new(physique, body.fk(pose));
    scene
        .inside(|_| true)
        .into_iter()
        .map(|((probe, obstacle), _)| ((probe, obstacle), scene.local(probe, obstacle)))
        .collect()
}

pub(crate) fn intrusions(
    physique: &Physique,
    body: &Body,
    poses: &[HumanPose],
    inside: &[Inside],
) -> Vec<Vec<Intrusion>> {
    let mut spans: Vec<(Pair, usize, usize)> = Vec::new();
    let mut open: HashMap<Pair, (usize, usize)> = HashMap::new();
    for (frame, pairs) in inside.iter().enumerate() {
        for (pair, _) in pairs {
            match open.entry(*pair) {
                Entry::Occupied(mut entry) if entry.get().1 + 1 == frame => {
                    entry.get_mut().1 = frame
                }
                Entry::Occupied(mut entry) => {
                    let (start, end) = entry.insert((frame, frame));
                    spans.push((*pair, start, end));
                }
                Entry::Vacant(entry) => {
                    entry.insert((frame, frame));
                }
            }
        }
    }
    spans.extend(
        open.into_iter()
            .map(|(pair, (start, end))| (pair, start, end)),
    );
    let mut scenes: HashMap<usize, Scene<'_>> = HashMap::new();
    let mut local_at = |frame: usize, (probe, obstacle): Pair| {
        scenes
            .entry(frame)
            .or_insert_with(|| Scene::new(physique, body.fk(&poses[frame])))
            .local(probe, obstacle)
    };
    let mut anchors: HashMap<(Pair, usize), Vec3> = HashMap::new();
    for (pair, start, end) in &spans {
        let sides = [
            start.checked_sub(1),
            Some(end + 1).filter(|after| *after < poses.len()),
        ];
        for frame in sides.into_iter().flatten() {
            anchors.insert((*pair, frame), local_at(frame, *pair));
        }
    }
    let locals: HashMap<(Pair, usize), Vec3> = inside
        .iter()
        .enumerate()
        .flat_map(|(frame, intrusions)| {
            intrusions
                .iter()
                .map(move |(pair, local)| ((*pair, frame), *local))
        })
        .collect();
    let mut per_frame: Vec<Vec<Intrusion>> = vec![Vec::new(); poses.len()];
    for (pair, start, end) in spans {
        let local = |frame: usize| locals[&(pair, frame)];
        let entry = start
            .checked_sub(1)
            .and_then(|before| anchors.get(&(pair, before)).copied());
        let leave = anchors.get(&(pair, end + 1)).copied();
        let suggested = |frame: usize| {
            let path = |anchor: Option<Vec3>, apart: usize| {
                anchor.filter(|_| apart < PATH_FRAMES).and_then(|anchor| {
                    let along = anchor - local(frame);
                    let length = along.length();
                    let reach = SHORTEST_PATH * physique.scale..LONGEST_PATH * physique.scale;
                    reach.contains(&length).then(|| along / length)
                })
            };
            path(entry, frame - start)
                .or_else(|| path(leave, end - frame))
                .unwrap_or_else(|| outward(physique, pair, local(frame)))
        };
        let frames: Vec<usize> = match entry.is_some() || leave.is_none() {
            true => (start..=end).collect(),
            false => (start..=end).rev().collect(),
        };
        let mut direction: Option<Vec3> = None;
        for frame in frames {
            let candidate = suggested(frame);
            let toward = match direction {
                Some(before) if before.dot(candidate) < COHERENT => before,
                _ => candidate,
            };
            direction = Some(toward);
            per_frame[frame].push(leaving(physique, pair, local(frame), toward));
        }
    }
    for intrusions in &mut per_frame {
        intrusions.sort_by_key(|intrusion| (intrusion.probe, intrusion.obstacle));
    }
    per_frame
}

fn outward(physique: &Physique, (_, obstacle): Pair, local: Vec3) -> Vec3 {
    let shell = &physique.obstacles[obstacle].shell;
    let nudge = NUDGE * physique.scale;
    let here = shell.distance(local);
    Vec3::new(
        shell.distance(local + Vec3::X * nudge) - here,
        shell.distance(local + Vec3::Y * nudge) - here,
        shell.distance(local + Vec3::Z * nudge) - here,
    )
    .normalize_or(Vec3::Y)
}

fn leaving(physique: &Physique, (probe, obstacle): Pair, local: Vec3, toward: Vec3) -> Intrusion {
    let shell = &physique.obstacles[obstacle].shell;
    let clearance = physique.clearance(probe, obstacle);
    let outside = |along: f32| shell.distance(local + toward * along) >= clearance;
    let longest = LONGEST_PATH * physique.scale;
    let mut high = FIRST_STRIDE * physique.scale;
    while !outside(high) && high < longest {
        high *= 2.0;
    }
    let mut low = 0.0;
    for _ in 0..BISECTIONS {
        let middle = (low + high) * 0.5;
        match outside(middle) {
            true => high = middle,
            false => low = middle,
        }
    }
    Intrusion {
        probe,
        obstacle,
        exit: local + toward * high,
        toward,
    }
}
