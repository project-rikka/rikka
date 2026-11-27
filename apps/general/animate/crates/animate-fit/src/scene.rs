use animate_body::{HumanBone, Worlds};
use rikka_model_core::glam::{Quat, Vec3};

use crate::physique::{Mask, Physique};
use crate::region::Region;

#[derive(Debug)]
pub(crate) struct Scene<'a> {
    physique: &'a Physique,
    worlds: Worlds,
    holders: Vec<Holder>,
}

#[derive(Clone, Copy, Debug)]
struct Holder {
    position: Vec3,
    inverse: Quat,
}

impl Holder {
    fn local(&self, point: Vec3) -> Vec3 {
        self.inverse * (point - self.position)
    }
}

pub(crate) type Pair = (usize, usize);

impl<'a> Scene<'a> {
    pub(crate) fn new(physique: &'a Physique, worlds: Worlds) -> Self {
        let holders = physique
            .obstacles
            .iter()
            .map(|obstacle| {
                let joint = worlds.joint(obstacle.bone);
                Holder {
                    position: joint.position,
                    inverse: joint.rotation.inverse(),
                }
            })
            .collect();
        Self {
            physique,
            worlds,
            holders,
        }
    }

    fn probe(&self, index: usize) -> Vec3 {
        let probe = &self.physique.probes[index];
        self.worlds.point(probe.bone, probe.local)
    }

    pub(crate) fn position(&self, bone: HumanBone) -> Vec3 {
        self.worlds.position(bone)
    }

    pub(crate) fn local(&self, probe: usize, obstacle: usize) -> Vec3 {
        self.holders[obstacle].local(self.probe(probe))
    }

    pub(crate) fn depth(&self, probe: usize, obstacle: usize) -> f32 {
        self.depth_at(probe, obstacle, self.local(probe, obstacle))
    }

    pub(crate) fn depth_at(&self, probe: usize, obstacle: usize, local: Vec3) -> f32 {
        let clearance = self.physique.clearance(probe, obstacle);
        let shape = &self.physique.obstacles[obstacle];
        let outside = local.distance(shape.centre) - shape.radius;
        let distance = match outside > clearance.max(0.0) {
            true => outside,
            false => shape.shell.distance(local),
        };
        clearance - distance
    }

    fn intrusion_at(&self, probe: usize, obstacle: usize, local: Vec3) -> Option<f32> {
        let clearance = self.physique.clearance(probe, obstacle);
        let shape = &self.physique.obstacles[obstacle];
        let outside = local.distance(shape.centre) - shape.radius;
        if outside > clearance.max(0.0) || shape.shell.clears(local, clearance) {
            return None;
        }
        let depth = clearance - shape.shell.distance(local);
        (depth > 0.0).then_some(depth)
    }

    pub(crate) fn inside(&self, desired: impl Fn(Region) -> bool) -> Vec<(Pair, f32)> {
        let margin = self.physique.cull_margin();
        let mut pairs = Vec::new();
        for cluster in self
            .physique
            .clusters
            .iter()
            .filter(|cluster| desired(cluster.region))
        {
            let centre = self.worlds.point(cluster.bone, cluster.centre);
            let near = bits(cluster.facing)
                .filter(|obstacle| {
                    let shape = &self.physique.obstacles[*obstacle];
                    let apart = self.holders[*obstacle].local(centre).distance(shape.centre);
                    apart - shape.radius - cluster.radius <= margin
                })
                .fold(0, |mask: Mask, obstacle| mask | 1 << obstacle);
            if near == 0 {
                continue;
            }
            for probe in cluster.probes.clone() {
                let point = self.probe(probe);
                for obstacle in bits(self.physique.probes[probe].facing & near) {
                    let local = self.holders[obstacle].local(point);
                    if let Some(depth) = self.intrusion_at(probe, obstacle, local) {
                        pairs.push(((probe, obstacle), depth));
                    }
                }
            }
        }
        pairs
    }

    pub(crate) fn touching(&self, probe: usize, within: f32) -> Mask {
        bits(self.physique.probes[probe].facing)
            .filter(|obstacle| self.depth(probe, *obstacle) > -within)
            .fold(0, |mask, obstacle| mask | 1 << obstacle)
    }
}

fn bits(mask: Mask) -> impl Iterator<Item = usize> {
    let mut rest = mask;
    std::iter::from_fn(move || {
        (rest != 0).then(|| {
            let index = rest.trailing_zeros() as usize;
            rest &= rest - 1;
            index
        })
    })
}
