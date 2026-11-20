use std::collections::{HashMap, HashSet};
use std::ops::Range;

use animate_body::{Body, HumanBone, Joint, Worlds};
use rikka_model_core::glam::Vec3;
use rikka_model_core::{Model, NodeId};
use rikka_physics_tailor::Wearer;

use crate::garment;
use crate::region::{Cover, Region};
use crate::scene::Scene;
use crate::shell::Shell;
use crate::surface::samples;

const ARM_PROBE_CELL: f32 = 0.015;
const LEG_PROBE_CELL: f32 = 0.045;
pub(crate) const SLICE: f32 = 0.015;
pub(crate) const SAMPLE: f32 = 0.006;
const RIGID_SHARE: f32 = 0.5;
const SHOULDER_SHARE: f32 = 0.4;
const GAP: f32 = 0.003;
const FLESH_GIVE: f32 = 0.045;
const SPARED_WITHIN: f32 = 2.0;

const SEGMENT_MARGIN: f32 = 0.02;
const PELVIS_DEPTH: f32 = 0.6;
pub(crate) const SHELLED: [HumanBone; 10] = [
    HumanBone::Hips,
    HumanBone::Spine,
    HumanBone::Chest,
    HumanBone::UpperChest,
    HumanBone::Neck,
    HumanBone::Head,
    HumanBone::LeftUpperLeg,
    HumanBone::RightUpperLeg,
    HumanBone::LeftLowerLeg,
    HumanBone::RightLowerLeg,
];

pub(crate) type Mask = u64;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Obstacle {
    pub(crate) region: Region,
    pub(crate) cover: Cover,
    pub(crate) bone: HumanBone,
    pub(crate) shell: Shell,
    pub(crate) centre: Vec3,
    pub(crate) radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Probe {
    pub(crate) region: Region,
    pub(crate) bone: HumanBone,
    pub(crate) local: Vec3,
    pub(crate) facing: Mask,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cluster {
    pub(crate) region: Region,
    pub(crate) bone: HumanBone,
    pub(crate) probes: Range<usize>,
    pub(crate) centre: Vec3,
    pub(crate) radius: f32,
    pub(crate) facing: Mask,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Physique {
    pub(crate) obstacles: Vec<Obstacle>,
    pub(crate) probes: Vec<Probe>,
    pub(crate) clusters: Vec<Cluster>,
    pub(crate) scale: f32,
}

impl Physique {
    #[must_use]
    pub fn new(body: &Body, model: &Model, wearer: &Wearer) -> Self {
        let scale = body.scale();
        let rest = body.fk(body.rest_pose());
        let folded = body.mapping().folded(model);
        let skin = &wearer.body;
        let parts: Vec<Part> = skin
            .vertices
            .iter()
            .map(|vertex| strongest(&vertex.joints, &vertex.weights, &folded))
            .collect();
        let flesh = flesh(skin, &parts, scale);
        let mut obstacles = obstacles(body, &rest, &flesh, scale);
        obstacles.extend(garment::obstacles(
            body,
            &rest,
            &wearer.garments,
            &folded,
            &flesh,
            scale,
        ));
        let mut physique = Self {
            probes: probes(body, &rest, skin, &parts, &obstacles, scale),
            obstacles,
            clusters: Vec::new(),
            scale,
        };
        physique.spare(&rest);
        physique.clusters = clusters(&physique.probes);
        physique
    }

    pub fn reaching<'a>(&'a self, worlds: &'a Worlds) -> impl Iterator<Item = Vec3> + 'a {
        self.probes
            .iter()
            .filter(|probe| matches!(probe.region, Region::LowerArm(_) | Region::Hand(_)))
            .map(|probe| worlds.point(probe.bone, probe.local))
    }

    pub(crate) fn clearance(&self, probe: usize, obstacle: usize) -> f32 {
        let obstacle = &self.obstacles[obstacle];
        let fleshy = self.probes[probe].region.is_leg()
            && obstacle.region.is_leg()
            && obstacle.cover == Cover::Skin;
        self.scale
            * match fleshy {
                true => -FLESH_GIVE,
                false => GAP,
            }
    }

    pub(crate) fn cull_margin(&self) -> f32 {
        GAP * self.scale
    }

    fn spare(&mut self, rest: &Worlds) {
        let scene = Scene::new(self, rest.clone());
        let within = SPARED_WITHIN * GAP * self.scale;
        let touching: Vec<Mask> = (0..self.probes.len())
            .map(|probe| scene.touching(probe, within))
            .collect();
        for (probe, touching) in self.probes.iter_mut().zip(touching) {
            probe.facing &= !touching;
        }
    }
}

pub(crate) type Part = Option<(HumanBone, f32)>;

pub(crate) struct Flesh {
    pub(crate) bone: HumanBone,
    pub(crate) point: Vec3,
}

pub(crate) fn strongest(
    joints: &[NodeId; 4],
    weights: &[f32; 4],
    folded: &[Option<HumanBone>],
) -> Part {
    let (slot, weight) = weights
        .iter()
        .enumerate()
        .max_by(|one, other| one.1.total_cmp(other.1))?;
    let bone = folded.get(joints[slot].index()).copied().flatten()?;
    Some((bone, *weight))
}

fn flesh(skin: &rikka_physics_tailor::Body, parts: &[Part], scale: f32) -> Vec<Flesh> {
    let mut flesh = Vec::new();
    for triangle in &skin.triangles {
        let corners = triangle.map(|corner| skin.vertices[corner as usize].rest);
        for sample in samples(corners, SAMPLE * scale) {
            let part = parts[triangle[sample.nearest()] as usize];
            if let Some((bone, _)) = part.filter(|(bone, _)| !Region::for_bone(*bone).is_arm()) {
                flesh.push(Flesh {
                    bone,
                    point: sample.point,
                });
            }
        }
    }
    flesh
}

fn obstacles(body: &Body, rest: &Worlds, flesh: &[Flesh], scale: f32) -> Vec<Obstacle> {
    let mut points: HashMap<HumanBone, Vec<Vec3>> = HashMap::new();
    for flesh in flesh.iter().filter(|flesh| SHELLED.contains(&flesh.bone)) {
        points.entry(flesh.bone).or_default().push(flesh.point);
    }
    SHELLED
        .into_iter()
        .filter_map(|bone| {
            let joint = rest.get(bone)?;
            let axis = body.direction(bone);
            let (low, high) = segment(body, bone, scale);
            let local: Vec<Vec3> = points
                .get(&bone)?
                .iter()
                .map(|point| joint.local(*point))
                .filter(|local| (low..=high).contains(&local.dot(axis)))
                .collect();
            let shell = Shell::closed(axis, SLICE * scale, &local)?;
            let (centre, radius) = shell.bound();
            Some(Obstacle {
                region: Region::for_bone(bone),
                cover: Cover::Skin,
                bone,
                shell,
                centre,
                radius,
            })
        })
        .collect()
}

fn segment(body: &Body, bone: HumanBone, scale: f32) -> (f32, f32) {
    use HumanBone as B;
    let margin = SEGMENT_MARGIN * scale;
    let children: &[HumanBone] = match bone {
        B::Hips => &[B::Spine],
        B::Spine => &[B::Chest, B::UpperChest, B::Neck],
        B::Chest => &[B::UpperChest, B::Neck],
        B::UpperChest => &[B::Neck],
        B::Neck => &[B::Head],
        B::LeftUpperLeg => &[B::LeftLowerLeg],
        B::RightUpperLeg => &[B::RightLowerLeg],
        B::LeftLowerLeg => &[B::LeftFoot],
        B::RightLowerLeg => &[B::RightFoot],
        _ => &[],
    };
    let high = children
        .iter()
        .find(|child| body.has(**child) && body.parent(**child) == Some(bone))
        .map_or(f32::MAX, |child| body.offset(*child).length() + margin);
    let low = match bone {
        B::Hips => -PELVIS_DEPTH * body.offset(B::LeftLowerLeg).length(),
        _ => -margin,
    };
    (low, high)
}

fn probes(
    body: &Body,
    rest: &Worlds,
    skin: &rikka_physics_tailor::Body,
    parts: &[Part],
    obstacles: &[Obstacle],
    scale: f32,
) -> Vec<Probe> {
    let mut occupied: HashSet<(HumanBone, [i32; 3])> = HashSet::new();
    let mut probes = Vec::new();
    for (vertex, part) in skin.vertices.iter().zip(parts) {
        let Some((bone, weight)) = *part else {
            continue;
        };
        let region = Region::for_bone(bone);
        if weight < RIGID_SHARE || !probing(region, bone) {
            continue;
        }
        let Some(joint) = rest.get(bone) else {
            continue;
        };
        if near_shoulder(body, joint, region, bone, vertex.rest) {
            continue;
        }
        let cell = scale
            * match region.is_leg() {
                true => LEG_PROBE_CELL,
                false => ARM_PROBE_CELL,
            };
        let key = (vertex.rest / cell).floor().as_ivec3().to_array();
        if !occupied.insert((bone, key)) {
            continue;
        }
        let facing = obstacles
            .iter()
            .enumerate()
            .filter(|(_, obstacle)| region.avoids(obstacle.cover) & obstacle.region.bit() != 0)
            .fold(0, |mask: Mask, (index, _)| mask | 1 << index);
        probes.push(Probe {
            region,
            bone,
            local: joint.local(vertex.rest),
            facing,
        });
    }
    probes.sort_by_key(|probe| probe.bone);
    probes
}

fn clusters(probes: &[Probe]) -> Vec<Cluster> {
    probes
        .chunk_by(|one, other| one.bone == other.bone)
        .scan(0, |start, count| {
            let range = *start..*start + count.len();
            *start = range.end;
            let centre = count.iter().map(|probe| probe.local).sum::<Vec3>() / count.len() as f32;
            Some(Cluster {
                region: count[0].region,
                bone: count[0].bone,
                probes: range,
                centre,
                radius: count
                    .iter()
                    .map(|probe| probe.local.distance(centre))
                    .fold(0.0, f32::max),
                facing: count.iter().fold(0, |mask, probe| mask | probe.facing),
            })
        })
        .collect()
}

fn probing(region: Region, bone: HumanBone) -> bool {
    match region {
        Region::Hand(side) => bone == side.hand(),
        _ => region.probes(),
    }
}

fn near_shoulder(body: &Body, joint: Joint, region: Region, bone: HumanBone, point: Vec3) -> bool {
    let Region::UpperArm(side) = region else {
        return false;
    };
    let along = joint.local(point).dot(body.direction(bone));
    along < SHOULDER_SHARE * body.offset(side.arm().chain()[1]).length()
}
