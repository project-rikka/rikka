use std::sync::Arc;

use animate_cast::FigureId;
use animate_cloth::{Crossing, Ray};
use rikka_model::glam::Vec3;
use rikka_physics_tailor::{ClothAsset, Piece};
use rikka_types::Point;

use crate::lens::Lens;

const FRONT_BAND: f32 = 0.05;
const NEAR_REACH: f32 = 10.0;

#[derive(Clone, Debug)]
pub struct FigureParticles {
    pub id: FigureId,
    pub name: String,
    pub cloth_asset: Arc<ClothAsset>,
    pub positions: Vec<Vec3>,
}

#[derive(Clone, Debug, Default)]
pub struct ClothSnapshot {
    pub figures: Vec<FigureParticles>,
}

#[derive(Debug)]
pub struct Spot {
    pub figure: FigureId,
    pub name: String,
    pub piece: String,
    pub rest: Vec3,
    pub world: Vec3,
}

impl FigureParticles {
    fn pieces(&self) -> impl Iterator<Item = (&Piece, &[Vec3])> + '_ {
        let mut start = 0;
        self.cloth_asset.pieces.iter().filter_map(move |piece| {
            let end = start + piece.particles.len();
            let positions = self.positions.get(start..end);
            start = end;
            Some((piece, positions?))
        })
    }

    fn spot(&self, piece: &Piece, rest: Vec3, world: Vec3) -> Spot {
        Spot {
            figure: self.id,
            name: self.name.clone(),
            piece: piece.name.clone(),
            rest,
            world,
        }
    }

    fn ray_hit(&self, ray: &Ray) -> Option<(f32, Spot)> {
        self.pieces()
            .flat_map(|(piece, positions)| {
                piece.triangles.iter().filter_map(move |corners| {
                    let corner = |at: usize| {
                        let index = corners[at] as usize;
                        Some((*positions.get(index)?, piece.particles.get(index)?.rest))
                    };
                    let [(a, ra), (b, rb), (c, rc)] = [corner(0)?, corner(1)?, corner(2)?];
                    let Crossing { along, u, v } = ray.crossing([a, b, c])?;
                    let rest = ra * (1.0 - u - v) + rb * u + rc * v;
                    let world = ray.at(along);
                    Some((along, self.spot(piece, rest, world)))
                })
            })
            .min_by(|one, two| one.0.total_cmp(&two.0))
    }

    fn free_particles(&self) -> impl Iterator<Item = (&Piece, Vec3, Vec3)> + '_ {
        self.pieces().flat_map(|(piece, positions)| {
            piece
                .particles
                .iter()
                .zip(positions)
                .filter(|(particle, _)| !particle.pinned)
                .map(move |(particle, world)| (piece, particle.rest, *world))
        })
    }

    fn particle_position(&self, piece: usize, particle: usize) -> Option<Vec3> {
        self.pieces().nth(piece)?.1.get(particle).copied()
    }
}

struct NearParticle<'a> {
    figure: &'a FigureParticles,
    piece: &'a Piece,
    rest: Vec3,
    world: Vec3,
    screen_distance: f32,
    depth: f32,
}

impl ClothSnapshot {
    pub fn spot_at(&self, lens: &Lens, at: Point) -> Option<Spot> {
        let (origin, direction) = lens.ray(at);
        let ray = Ray { origin, direction };
        self.figures
            .iter()
            .filter_map(|figure| figure.ray_hit(&ray))
            .min_by(|one, two| one.0.total_cmp(&two.0))
            .map(|(_, spot)| spot)
            .or_else(|| self.nearest_spot(lens, &ray, at))
    }

    fn nearest_spot(&self, lens: &Lens, ray: &Ray, at: Point) -> Option<Spot> {
        let candidates: Vec<NearParticle<'_>> = self
            .figures
            .iter()
            .flat_map(|figure| {
                figure
                    .free_particles()
                    .filter_map(move |(piece, rest, world)| {
                        let screen_distance = lens.project(world)?.distance(at);
                        (screen_distance <= NEAR_REACH).then(|| NearParticle {
                            figure,
                            piece,
                            rest,
                            world,
                            screen_distance,
                            depth: (world - ray.origin).dot(ray.direction),
                        })
                    })
            })
            .collect();
        let front = candidates
            .iter()
            .map(|candidate| candidate.depth)
            .fold(f32::INFINITY, f32::min);
        candidates
            .into_iter()
            .filter(|candidate| candidate.depth <= front + FRONT_BAND)
            .min_by(|one, two| one.screen_distance.total_cmp(&two.screen_distance))
            .map(|nearest| {
                nearest
                    .figure
                    .spot(nearest.piece, nearest.rest, nearest.world)
            })
    }

    pub fn patch(&self, spot: &Spot, reach: f32) -> Vec<(Vec3, f32)> {
        let Some(figure) = self.figures.iter().find(|figure| figure.id == spot.figure) else {
            return Vec::new();
        };
        let Some(pick) = figure.cloth_asset.pick(spot.rest, reach) else {
            return Vec::new();
        };
        pick.members
            .iter()
            .filter_map(|member| {
                Some((
                    figure.particle_position(pick.piece, member.particle)?,
                    member.weight,
                ))
            })
            .collect()
    }
}
