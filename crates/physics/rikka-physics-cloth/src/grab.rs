use std::collections::BTreeMap;

use glam::Vec3;
use rikka_physics_tailor::{ClothAsset, Grab, MAX_GRABS};

use crate::error::{Error, Result};
use crate::figure::FigureId;
use crate::layout::Topology;

pub(crate) const MAX_GRABBED: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GrabPoint {
    id: u64,
    point: Vec3,
    strength: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GrabState {
    points: Vec<GrabPoint>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GrabTransition {
    from: Vec<GrabPoint>,
    to: Vec<GrabPoint>,
    pub(crate) entries: Vec<[u32; 4]>,
    pub(crate) pieces: Vec<f32>,
}

impl GrabState {
    pub(crate) fn unchanged(&self, grabs: &[&Grab]) -> bool {
        self.points.len() == grabs.len()
            && self.points.iter().zip(grabs).all(|(point, grab)| {
                point.id == grab.id && point.point == grab.point && point.strength == strength(grab)
            })
    }

    pub(crate) fn plan(&mut self, topology: &Topology, grabs: &[&Grab]) -> GrabTransition {
        let to: Vec<GrabPoint> = grabs
            .iter()
            .map(|grab| GrabPoint {
                id: grab.id,
                point: grab.point,
                strength: strength(grab),
            })
            .collect();
        let from: Vec<GrabPoint> = to
            .iter()
            .map(|now| {
                self.points
                    .iter()
                    .find(|point| point.id == now.id)
                    .copied()
                    .unwrap_or(*now)
            })
            .collect();
        self.points.clone_from(&to);
        let mut pieces = vec![0.0f32; topology.gpu.len()];
        let mut chosen: BTreeMap<u32, (f32, [u32; 4])> = BTreeMap::new();
        for (slot, grab) in grabs.iter().enumerate() {
            let strength = from[slot].strength.max(to[slot].strength);
            if strength <= 0.0 {
                continue;
            }
            let pick = &grab.pick;
            pieces[pick.piece] = pieces[pick.piece].max(strength);
            let order = &topology.gpu[pick.piece];
            let centre = order[pick.centre];
            for member in &pick.members {
                let particle = order[member.particle];
                let score = member.weight * strength;
                let entry = [particle, centre, slot as u32, member.weight.to_bits()];
                chosen
                    .entry(particle)
                    .and_modify(|best| {
                        if score > best.0 {
                            *best = (score, entry);
                        }
                    })
                    .or_insert((score, entry));
            }
        }
        let mut ranked: Vec<(f32, [u32; 4])> = chosen.into_values().collect();
        if ranked.len() > MAX_GRABBED {
            ranked.sort_by(|one, two| two.0.total_cmp(&one.0).then(one.1[0].cmp(&two.1[0])));
            ranked.truncate(MAX_GRABBED);
            ranked.sort_by_key(|(_, entry)| entry[0]);
        }
        GrabTransition {
            from,
            to,
            entries: ranked.into_iter().map(|(_, entry)| entry).collect(),
            pieces,
        }
    }

    pub(crate) fn release(&mut self) {
        self.points.clear();
    }
}

fn strength(grab: &Grab) -> f32 {
    grab.strength.clamp(0.0, 1.0)
}

impl GrabTransition {
    pub(crate) fn slots(&self, epochs: u32, warming: Option<[f32; 2]>) -> Vec<[f32; 4]> {
        let mut slots = vec![[0.0; 4]; (epochs as usize + 1) * MAX_GRABS];
        for boundary in 0..=epochs {
            let share = boundary as f32 / epochs as f32;
            for (slot, (from, to)) in self.from.iter().zip(&self.to).enumerate() {
                let (point, strength) = match warming {
                    Some([start, end]) => (to.point, to.strength * (start + (end - start) * share)),
                    None => (
                        from.point.lerp(to.point, share),
                        from.strength + (to.strength - from.strength) * share,
                    ),
                };
                slots[boundary as usize * MAX_GRABS + slot] = point.extend(strength).to_array();
            }
        }
        slots
    }
}

pub(crate) fn validate(figure: FigureId, grabs: &[&Grab], asset: &ClothAsset) -> Result<()> {
    if grabs.len() > MAX_GRABS {
        return Err(Error::TooManyGrabs {
            figure,
            count: grabs.len(),
        });
    }
    for grab in grabs {
        if !grab.point.is_finite() || !grab.strength.is_finite() {
            return Err(Error::NonFiniteGrab(figure));
        }
        let pick = &grab.pick;
        let Some(piece) = asset.pieces.get(pick.piece) else {
            return Err(Error::UnknownPiece {
                figure,
                piece: pick.piece,
            });
        };
        let particles = piece.particles.len();
        let outside = pick.centre >= particles
            || pick
                .members
                .iter()
                .any(|member| member.particle >= particles || !member.weight.is_finite());
        if outside {
            return Err(Error::GrabOutsidePiece {
                figure,
                piece: pick.piece,
            });
        }
    }
    Ok(())
}
