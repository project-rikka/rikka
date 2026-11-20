use std::cmp::Ordering;

use rikka_model_core::Bounds;

use crate::asset::{Piece, PieceKind};
use crate::geometry::Shell;

const OVERLAP: f32 = 0.01;
const SURFACE_REACH: f32 = 0.06;
const SURFACE_MARGIN: f32 = 0.001;
const MIN_SURFACE_VOTES: usize = 4;

pub(crate) fn layer(pieces: &mut [Piece], overrides: &[Option<u8>]) {
    let count = pieces.len();
    let summary: Vec<(bool, f32, Bounds)> = pieces
        .iter()
        .map(|piece| {
            let mut clearances: Vec<f32> = piece
                .particles
                .iter()
                .filter(|particle| !particle.pinned)
                .map(|particle| particle.clearance)
                .collect();
            clearances.sort_by(f32::total_cmp);
            let median = clearances.get(clearances.len() / 2).copied().unwrap_or(0.0);
            let bounds = Bounds::from_points(piece.particles.iter().map(|particle| particle.rest));
            (piece.kind == PieceKind::Strands, median, bounds)
        })
        .collect();
    let mut fallback: Vec<usize> = (0..count).collect();
    fallback.sort_by(|a, b| {
        summary[*a]
            .0
            .cmp(&summary[*b].0)
            .then(summary[*a].1.total_cmp(&summary[*b].1))
            .then(a.cmp(b))
    });
    let mut rank = vec![0usize; count];
    for (at, &index) in fallback.iter().enumerate() {
        rank[index] = at;
    }
    let mut shells: Vec<Option<Shell>> = vec![None; count];
    let mut above: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut indegree = vec![0usize; count];
    for a in 0..count {
        for b in a + 1..count {
            if !summary[a].2.overlaps(&summary[b].2, OVERLAP) {
                continue;
            }
            let (below, over) = match sheet_order(pieces, &mut shells, &summary, a, b) {
                Some(Ordering::Less) => (a, b),
                Some(Ordering::Greater) => (b, a),
                _ if rank[a] < rank[b] => (a, b),
                _ => (b, a),
            };
            above[below].push(over);
            indegree[over] += 1;
        }
    }
    let mut placed = vec![false; count];
    let mut layers = vec![0u8; count];
    for _ in 0..count {
        let next = (0..count)
            .filter(|&index| !placed[index] && indegree[index] == 0)
            .min_by_key(|&index| rank[index])
            .or_else(|| {
                (0..count)
                    .filter(|&index| !placed[index])
                    .min_by_key(|&index| rank[index])
            });
        let Some(index) = next else {
            break;
        };
        placed[index] = true;
        let next_layer = layers[index].saturating_add(1);
        for &over in &above[index] {
            if !placed[over] {
                layers[over] = layers[over].max(next_layer);
                indegree[over] = indegree[over].saturating_sub(1);
            }
        }
    }
    for ((piece, layer), chosen) in pieces.iter_mut().zip(layers).zip(overrides) {
        piece.layer = chosen.unwrap_or(layer);
    }
}

fn sheet_order(
    pieces: &[Piece],
    shells: &mut [Option<Shell>],
    summary: &[(bool, f32, Bounds)],
    a: usize,
    b: usize,
) -> Option<Ordering> {
    let is_sheet = |index: usize| {
        pieces[index].kind == PieceKind::Sheet && !pieces[index].triangles.is_empty()
    };
    if !is_sheet(a) || !is_sheet(b) {
        return None;
    }
    let shell_a = shells[a]
        .get_or_insert_with(|| Shell::new(&pieces[a].rest_positions(), &pieces[a].triangles));
    let (b_outside_a, b_inside_a) = side_votes(&pieces[b], shell_a, &summary[a].2);
    let shell_b = shells[b]
        .get_or_insert_with(|| Shell::new(&pieces[b].rest_positions(), &pieces[b].triangles));
    let (a_outside_b, a_inside_b) = side_votes(&pieces[a], shell_b, &summary[b].2);
    let b_over_a = b_outside_a + a_inside_b;
    let a_over_b = a_outside_b + b_inside_a;
    if b_over_a + a_over_b < MIN_SURFACE_VOTES {
        return None;
    }
    if b_over_a >= 2 * a_over_b {
        Some(Ordering::Less)
    } else if a_over_b >= 2 * b_over_a {
        Some(Ordering::Greater)
    } else {
        None
    }
}

fn side_votes(piece: &Piece, shell: &Shell, bounds: &Bounds) -> (usize, usize) {
    let low = bounds.min - SURFACE_REACH;
    let high = bounds.max + SURFACE_REACH;
    let mut outside = 0usize;
    let mut inside = 0usize;
    for particle in &piece.particles {
        if particle.pinned {
            continue;
        }
        let point = particle.rest;
        if !(point.cmpge(low).all() && point.cmple(high).all()) {
            continue;
        }
        let Some(signed) = shell.signed(point, SURFACE_REACH) else {
            continue;
        };
        if signed.distance > SURFACE_MARGIN {
            outside += 1;
        } else if signed.distance < -SURFACE_MARGIN {
            inside += 1;
        }
    }
    (outside, inside)
}
