use glam::Vec3;

use crate::asset::{BindTarget, Piece, PieceKind, Underlay};
use crate::geometry::{Feature, TriangleTree};

const REACH: f32 = 0.1;
const CANDIDATES: usize = 4;

pub(crate) fn underlay(pieces: &mut [Piece]) {
    let trees: Vec<Option<TriangleTree>> = pieces
        .iter()
        .map(|piece| {
            (piece.kind == PieceKind::Sheet).then(|| {
                TriangleTree::new(
                    piece
                        .triangles
                        .iter()
                        .map(|triangle| {
                            triangle.map(|corner| piece.particles[corner as usize].rest)
                        })
                        .collect(),
                )
            })
        })
        .collect();
    let unders: Vec<Vec<Option<Underlay>>> = pieces
        .iter()
        .map(|piece| {
            let inner: Vec<usize> = (0..pieces.len())
                .filter(|&other| pieces[other].layer < piece.layer && trees[other].is_some())
                .collect();
            if piece.kind != PieceKind::Sheet || inner.is_empty() {
                return vec![None; piece.bindings.len()];
            }
            let drawn = piece.drape(&piece.rest_positions());
            piece
                .bindings
                .iter()
                .zip(&drawn)
                .map(|(bind, drawn)| match bind.target {
                    BindTarget::Triangles(_) => nearest_under(drawn.position, &inner, &trees),
                    BindTarget::Strand(_) => None,
                })
                .collect()
        })
        .collect();
    for (piece, unders) in pieces.iter_mut().zip(unders) {
        for (bind, under) in piece.bindings.iter_mut().zip(unders) {
            bind.underlay = under;
        }
    }
}

fn nearest_under(point: Vec3, inner: &[usize], trees: &[Option<TriangleTree>]) -> Option<Underlay> {
    inner
        .iter()
        .filter_map(|&piece| {
            let tree = trees[piece].as_ref()?;
            let hits = tree.nearest_few_where(point, CANDIDATES, REACH, |_| true);
            hits.iter()
                .find(|hit| matches!(hit.closest.feature, Feature::Face))
                .or(hits.first())
                .map(|hit| (hit.distance, piece, hit.triangle))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, piece, triangle)| Underlay {
            piece: piece as u32,
            triangle,
        })
}
