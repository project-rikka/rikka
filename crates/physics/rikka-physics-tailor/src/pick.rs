use glam::Vec3;

use crate::asset::ClothAsset;

const MIN_WEIGHT: f32 = 0.02;
const LAYER_TIE: f32 = 0.003;

#[derive(Clone, Debug, PartialEq)]
pub struct ParticlePick {
    pub piece: usize,
    pub centre: usize,
    pub members: Vec<PickWeight>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickWeight {
    pub particle: usize,
    pub weight: f32,
}

impl ClothAsset {
    #[must_use]
    pub fn pick(&self, at: Vec3, radius: f32) -> Option<ParticlePick> {
        if radius.is_nan() || radius <= 0.0 || !at.is_finite() {
            return None;
        }
        let (piece, centre, distance) = self
            .pieces
            .iter()
            .enumerate()
            .flat_map(|(index, piece)| {
                piece
                    .particles
                    .iter()
                    .enumerate()
                    .filter(|(_, particle)| !particle.pinned)
                    .map(move |(particle, data)| (index, particle, data.rest.distance(at)))
            })
            .min_by(|one, two| {
                let layered = |(piece, _, distance): &(usize, usize, f32)| {
                    distance - LAYER_TIE * f32::from(self.pieces[*piece].layer)
                };
                layered(one).total_cmp(&layered(two))
            })?;
        if distance > radius {
            return None;
        }
        let piece_data = &self.pieces[piece];
        let middle = piece_data.particles[centre].rest;
        let members = piece_data
            .particles
            .iter()
            .enumerate()
            .filter(|(_, particle)| !particle.pinned)
            .filter_map(|(particle, data)| {
                let weight = if particle == centre {
                    1.0
                } else {
                    falloff(data.rest.distance(middle) / radius)
                };
                (weight >= MIN_WEIGHT).then_some(PickWeight { particle, weight })
            })
            .collect();
        Some(ParticlePick {
            piece,
            centre,
            members,
        })
    }
}

fn falloff(share: f32) -> f32 {
    let inside = (1.0 - share * share).max(0.0);
    inside * inside
}
