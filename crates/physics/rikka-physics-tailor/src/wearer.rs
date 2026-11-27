use glam::Vec3;
use rikka_humanoid::Anatomy;
use rikka_model_core::{Model, NodeId};

use crate::analysis::{AnalyzedPiece, ClothAnalysis};
use crate::asset::{Body, PieceKey, PieceKind};
use crate::cloth_settings::Role;
use crate::constraints::pin_loose_parts;
use crate::error::Result;
use crate::figure::Figure;
use crate::geometry::Graph;
use crate::preset::Preset;

#[derive(Clone, Debug, PartialEq)]
pub struct Wearer {
    pub body: Body,
    pub garments: Vec<Garment>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Garment {
    pub piece: PieceKey,
    pub preset: Preset,
    pub vertices: Vec<GarmentVertex>,
    pub triangles: Vec<[u32; 3]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GarmentVertex {
    pub rest: Vec3,
    pub joints: [NodeId; 4],
    pub weights: [f32; 4],
    pub give: f32,
}

impl Wearer {
    pub fn from_model(
        model: &Model,
        anatomy: &Anatomy,
        analysis: &ClothAnalysis,
    ) -> Result<Wearer> {
        let figure = Figure::new(model, anatomy)?;
        let pieces: Vec<&AnalyzedPiece> = analysis
            .pieces()
            .iter()
            .filter(|piece| piece.kind == PieceKind::Sheet)
            .collect();
        let mut slot = vec![None; figure.vertices.len()];
        for (index, piece) in pieces.iter().enumerate() {
            for (at, vertex) in piece.vertices.iter().enumerate() {
                slot[*vertex as usize] = Some((index, at as u32));
            }
        }
        let mut triangles: Vec<Vec<[u32; 3]>> = vec![Vec::new(); pieces.len()];
        for triangle in &figure.triangles {
            let corners = triangle.corners.map(|corner| slot[corner as usize]);
            if let [
                Some((index, one)),
                Some((two_index, two)),
                Some((three_index, three)),
            ] = corners
                && index == two_index
                && index == three_index
            {
                triangles[index].push([one, two, three]);
            }
        }
        Ok(Wearer {
            body: analysis.skin_body(&figure).clone(),
            garments: pieces
                .into_iter()
                .zip(triangles)
                .map(|(piece, triangles)| garment(&figure, analysis, piece, triangles))
                .collect(),
        })
    }
}

fn garment(
    figure: &Figure<'_>,
    analysis: &ClothAnalysis,
    piece: &AnalyzedPiece,
    triangles: Vec<[u32; 3]>,
) -> Garment {
    let reaches = match piece.enabled {
        true => reaches(figure, analysis.roles(), &piece.vertices, &triangles),
        false => vec![0.0; piece.vertices.len()],
    };
    let deepest = reaches.iter().copied().fold(0.0f32, f32::max);
    let vertices = piece
        .vertices
        .iter()
        .zip(&reaches)
        .map(|(vertex, reach)| {
            let rest = &figure.vertices[*vertex as usize];
            let depth = match deepest > 0.0 {
                true => reach / deepest,
                false => 0.0,
            };
            GarmentVertex {
                rest: rest.position,
                joints: rest.joints,
                weights: rest.weights,
                give: piece.style.yield_at(
                    piece.kind,
                    *reach,
                    analysis.spreads()[*vertex as usize],
                    depth,
                ),
            }
        })
        .collect();
    Garment {
        piece: piece.key,
        preset: piece.preset,
        vertices,
        triangles,
    }
}

fn reaches(
    figure: &Figure<'_>,
    roles: &[Role],
    members: &[u32],
    triangles: &[[u32; 3]],
) -> Vec<f32> {
    let (labels, count) = figure.weld(members, figure.same_distance());
    let mut positions = vec![Vec3::ZERO; count];
    let mut pinned = vec![false; count];
    for (member, label) in members.iter().zip(&labels) {
        positions[*label as usize] = figure.vertices[*member as usize].position;
        pinned[*label as usize] |= roles[*member as usize] == Role::Pinned;
    }
    let graph = Graph::new(
        count,
        triangles.iter().flat_map(|triangle| {
            (0..3).map(|side| {
                let (one, two) = (
                    labels[triangle[side] as usize],
                    labels[triangle[(side + 1) % 3] as usize],
                );
                (
                    one,
                    two,
                    positions[one as usize].distance(positions[two as usize]),
                )
            })
        }),
    );
    pin_loose_parts(&graph, &positions, &mut pinned);
    let reached = graph.reach(
        (0..count as u32).filter(|label| pinned[*label as usize]),
        f32::INFINITY,
    );
    labels
        .iter()
        .map(|label| reached.distance[*label as usize])
        .collect()
}
