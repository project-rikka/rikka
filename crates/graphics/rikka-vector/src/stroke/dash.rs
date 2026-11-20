use crate::stroke::polyline::{Polyline, Vertex};

const SPENT: f32 = 1.0e-6;
const MAX_DASHES: f32 = (1 << 20) as f32;

struct Pattern<'a> {
    dashes: &'a [f32],
    index: usize,
    remaining: f32,
}

impl Pattern<'_> {
    fn period(dashes: &[f32]) -> f32 {
        dashes.iter().sum::<f32>() * if dashes.len() % 2 == 1 { 2.0 } else { 1.0 }
    }

    fn new(dashes: &[f32], offset: f32) -> Pattern<'_> {
        let mut pattern = Pattern {
            dashes,
            index: 0,
            remaining: dashes[0],
        };
        let mut skip = offset.rem_euclid(Pattern::period(dashes));
        while skip > 0.0 {
            if skip >= pattern.remaining {
                skip -= pattern.remaining;
                pattern.advance();
            } else {
                pattern.remaining -= skip;
                skip = 0.0;
            }
        }
        pattern
    }

    fn is_on(&self) -> bool {
        self.index.is_multiple_of(2)
    }

    fn advance(&mut self) {
        self.index = (self.index + 1) % (self.dashes.len() * 2);
        self.remaining = self.dashes[self.index % self.dashes.len()];
    }
}

pub(crate) fn each_dash(
    line: &Polyline,
    dashes: &[f32],
    offset: f32,
    piece: &mut Polyline,
    held: &mut Vec<Vertex>,
    mut emit: impl FnMut(&Polyline),
) {
    let vertices = &line.vertices;
    if vertices.len() < 2 {
        return;
    }
    let mut length: f32 = vertices
        .windows(2)
        .map(|pair| pair[0].at.distance(pair[1].at))
        .sum();
    if line.closed {
        length += vertices[vertices.len() - 1].at.distance(vertices[0].at);
    }
    if length / Pattern::period(dashes) * dashes.len() as f32 > MAX_DASHES {
        emit(line);
        return;
    }
    let mut pattern = Pattern::new(dashes, offset);
    let starts_on = pattern.is_on();
    let count = match line.closed {
        true => vertices.len(),
        false => vertices.len() - 1,
    };
    piece.vertices.clear();
    piece.closed = false;
    held.clear();
    let mut holding = line.closed && starts_on;
    if starts_on {
        piece.vertices.push(Vertex {
            at: vertices[0].at,
            smooth: false,
        });
    }
    let mut finish = |piece: &mut Polyline, held: &mut Vec<Vertex>, holding: &mut bool| {
        if *holding {
            held.extend_from_slice(&piece.vertices);
            *holding = false;
        } else if !piece.vertices.is_empty() {
            emit(piece);
        }
        piece.vertices.clear();
    };
    for segment in 0..count {
        let from = vertices[segment];
        let to = vertices[(segment + 1) % vertices.len()];
        let mut length = from.at.distance(to.at);
        let mut start = from.at;
        while length > 0.0 {
            let step = pattern.remaining.min(length);
            let reaches = step >= length;
            let at = match reaches {
                true => to.at,
                false => start + (to.at - start) * (step / length),
            };
            if pattern.is_on() && piece.vertices.last().is_none_or(|last| last.at != at) {
                piece.vertices.push(Vertex {
                    at,
                    smooth: reaches && to.smooth,
                });
            }
            length -= step;
            pattern.remaining -= step;
            start = at;
            if pattern.remaining <= SPENT {
                if pattern.is_on() {
                    finish(piece, held, &mut holding);
                }
                pattern.advance();
                if pattern.is_on() {
                    piece.vertices.push(Vertex {
                        at: start,
                        smooth: false,
                    });
                }
            }
        }
    }
    if line.closed && !held.is_empty() && pattern.is_on() && !piece.vertices.is_empty() {
        piece.vertices.extend_from_slice(&held[1..]);
        held.clear();
    }
    if piece.vertices.len() > 1 {
        emit(piece);
    }
    if held.len() > 1 {
        piece.vertices.clear();
        piece.vertices.extend_from_slice(held);
        emit(piece);
    }
}
