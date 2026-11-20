use animate_body::{HumanBone, Joint, Worlds};
use rikka_model_core::glam::Vec3;

use crate::physique::Physique;
use crate::region::Region;

#[derive(Debug)]
pub(crate) struct Scene<'a> {
    physique: &'a Physique,
    worlds: Worlds,
}

pub(crate) type Pair = (usize, usize);

impl<'a> Scene<'a> {
    pub(crate) fn new(physique: &'a Physique, worlds: Worlds) -> Self {
        Self { physique, worlds }
    }

    fn probe(&self, index: usize) -> Vec3 {
        let probe = &self.physique.probes[index];
        self.worlds.point(probe.bone, probe.local)
    }

    pub(crate) fn position(&self, bone: HumanBone) -> Vec3 {
        self.worlds.position(bone)
    }

    fn holder(&self, obstacle: usize) -> Joint {
        self.worlds.joint(self.physique.obstacles[obstacle].bone)
    }

    pub(crate) fn local(&self, probe: usize, obstacle: usize) -> Vec3 {
        self.holder(obstacle).local(self.probe(probe))
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
                    let apart = self.holder(*obstacle).local(centre).distance(shape.centre);
                    apart - shape.radius - cluster.radius <= margin
                })
                .fold(0, |mask, obstacle| mask | 1 << obstacle);
            if near == 0 {
                continue;
            }
            for probe in cluster.probes.clone() {
                for obstacle in bits(self.physique.probes[probe].facing & near) {
                    let depth = self.depth(probe, obstacle);
                    if depth > 0.0 {
                        pairs.push(((probe, obstacle), depth));
                    }
                }
            }
        }
        pairs
    }

    pub(crate) fn touching(&self, probe: usize, within: f32) -> u16 {
        bits(self.physique.probes[probe].facing)
            .filter(|obstacle| self.depth(probe, *obstacle) > -within)
            .fold(0, |mask, obstacle| mask | 1 << obstacle)
    }
}

fn bits(mask: u16) -> impl Iterator<Item = usize> {
    (0..u16::BITS as usize).filter(move |index| mask & 1 << index != 0)
}
