use rikka_types::Point;

use crate::flatten::flatness::Flatness;
use crate::lanes::LANES;
use crate::path::{Path, Verb};

pub const MAX_FLATTENED: usize = 1 << 18;

#[derive(Debug)]
pub struct Columns<'c> {
    pub xs: &'c mut Vec<f32>,
    pub ys: &'c mut Vec<f32>,
    pub ends: &'c mut Vec<usize>,
}

#[derive(Debug)]
pub struct Companions<'c> {
    pub corners: &'c [Point],
    pub xs: &'c mut Vec<f32>,
    pub ys: &'c mut Vec<f32>,
}

#[derive(Debug)]
pub struct Flattening<'f> {
    pub flatness: Flatness,
    pub limit: usize,
    pub steps: &'f mut Vec<u16>,
}

pub fn flatten(
    path: &Path,
    flattening: Flattening<'_>,
    columns: Columns<'_>,
    companions: Option<Companions<'_>>,
) -> bool {
    flatten_points(path.verbs(), path.points(), flattening, columns, companions)
}

pub(crate) fn flatten_points(
    verbs: &[Verb],
    points: &[Point],
    flattening: Flattening<'_>,
    columns: Columns<'_>,
    mut miters: Option<Companions<'_>>,
) -> bool {
    let Flattening {
        flatness,
        limit,
        steps,
    } = flattening;
    columns.ends.clear();
    let Some(len) = flattened_len(verbs, points, flatness, steps, limit) else {
        columns.xs.clear();
        columns.ys.clear();
        return false;
    };
    let padded = (len + LANES).next_multiple_of(LANES);
    reset(columns.xs, padded);
    reset(columns.ys, padded);
    if let Some(miters) = miters.as_mut() {
        reset(miters.xs, padded);
        reset(miters.ys, padded);
    }
    let mut writer = Writer {
        xs: &mut columns.xs[..len],
        ys: &mut columns.ys[..len],
        miters: miters
            .as_mut()
            .map(|miters| (miters.corners, &mut miters.xs[..len], &mut miters.ys[..len])),
        at: 0,
    };
    let mut index = 0;
    let mut start = None;
    let mut counts = steps.iter();
    for &verb in verbs {
        let from = index.max(1) - 1;
        match verb {
            Verb::Move => {
                if let Some(start) = start {
                    writer.point(points, start);
                    columns.ends.push(writer.at);
                }
                start = Some(index);
                writer.point(points, index);
                index += 1;
            }
            Verb::Line => {
                writer.point(points, index);
                index += 1;
            }
            Verb::Quad => {
                let ends = [from, index, index + 1];
                let count = counts.next().map_or(1, |&count| usize::from(count));
                writer.quad(points, ends, count);
                index += 2;
            }
            Verb::Cubic => {
                let ends = [from, index, index + 1, index + 2];
                let count = counts.next().map_or(1, |&count| usize::from(count));
                writer.cubic(points, ends, count);
                index += 3;
            }
            Verb::Close => {}
        }
    }
    if let Some(start) = start {
        writer.point(points, start);
        columns.ends.push(writer.at);
    }
    pad(columns.xs, len);
    pad(columns.ys, len);
    if let Some(miters) = miters {
        pad(miters.xs, len);
        pad(miters.ys, len);
    }
    true
}

fn flattened_len(
    verbs: &[Verb],
    points: &[Point],
    flatness: Flatness,
    steps: &mut Vec<u16>,
    limit: usize,
) -> Option<usize> {
    steps.clear();
    let mut len = 0usize;
    let mut index = 0;
    for &verb in verbs {
        let from = index.max(1) - 1;
        len += match verb {
            Verb::Move => {
                index += 1;
                2
            }
            Verb::Line => {
                index += 1;
                1
            }
            Verb::Quad => {
                let count = flatness.quad_steps(points[from], points[index], points[index + 1]);
                steps.push(count as u16);
                index += 2;
                count as usize
            }
            Verb::Cubic => {
                let count = flatness.cubic_steps(
                    points[from],
                    points[index],
                    points[index + 1],
                    points[index + 2],
                );
                steps.push(count as u16);
                index += 3;
                count as usize
            }
            Verb::Close => 0,
        };
        if len > limit {
            return None;
        }
    }
    Some(len)
}

struct Writer<'w> {
    xs: &'w mut [f32],
    ys: &'w mut [f32],
    miters: Option<(&'w [Point], &'w mut [f32], &'w mut [f32])>,
    at: usize,
}

impl Writer<'_> {
    #[inline(always)]
    fn point(&mut self, points: &[Point], index: usize) {
        self.xs[self.at] = points[index].x;
        self.ys[self.at] = points[index].y;
        if let Some((corners, xs, ys)) = self.miters.as_mut() {
            xs[self.at] = corners[index].x;
            ys[self.at] = corners[index].y;
        }
        self.at += 1;
    }

    #[inline(always)]
    fn quad(&mut self, points: &[Point], ends: [usize; 3], count: usize) {
        let span = self.at..self.at + count;
        quad(
            &mut self.xs[span.clone()],
            &mut self.ys[span.clone()],
            ends.map(|at| points[at]),
        );
        if let Some((corners, xs, ys)) = self.miters.as_mut() {
            quad(
                &mut xs[span.clone()],
                &mut ys[span.clone()],
                ends.map(|at| corners[at]),
            );
        }
        self.at = span.end;
    }

    #[inline(always)]
    fn cubic(&mut self, points: &[Point], ends: [usize; 4], count: usize) {
        let span = self.at..self.at + count;
        cubic(
            &mut self.xs[span.clone()],
            &mut self.ys[span.clone()],
            ends.map(|at| points[at]),
        );
        if let Some((corners, xs, ys)) = self.miters.as_mut() {
            cubic(
                &mut xs[span.clone()],
                &mut ys[span.clone()],
                ends.map(|at| corners[at]),
            );
        }
        self.at = span.end;
    }
}

#[inline(always)]
fn quad(xs: &mut [f32], ys: &mut [f32], [p0, p1, p2]: [Point; 3]) {
    let count = xs.len();
    let step = 1.0 / count as f32;
    let (ax, ay) = (2.0 * (p1.x - p0.x), 2.0 * (p1.y - p0.y));
    let (bx, by) = (p0.x - 2.0 * p1.x + p2.x, p0.y - 2.0 * p1.y + p2.y);
    let (Some((last_x, xs)), Some((last_y, ys))) = (xs.split_last_mut(), ys.split_last_mut())
    else {
        return;
    };
    for (index, (x, y)) in xs.iter_mut().zip(ys.iter_mut()).enumerate() {
        let t = (index + 1) as f32 * step;
        *x = p0.x + t * (ax + t * bx);
        *y = p0.y + t * (ay + t * by);
    }
    *last_x = p2.x;
    *last_y = p2.y;
}

#[inline(always)]
fn cubic(xs: &mut [f32], ys: &mut [f32], [p0, p1, p2, p3]: [Point; 4]) {
    let count = xs.len();
    let step = 1.0 / count as f32;
    let a = (3.0 * (p1.x - p0.x), 3.0 * (p1.y - p0.y));
    let b = (
        3.0 * (p0.x - 2.0 * p1.x + p2.x),
        3.0 * (p0.y - 2.0 * p1.y + p2.y),
    );
    let c = (
        p3.x - p0.x + 3.0 * (p1.x - p2.x),
        p3.y - p0.y + 3.0 * (p1.y - p2.y),
    );
    let (Some((last_x, xs)), Some((last_y, ys))) = (xs.split_last_mut(), ys.split_last_mut())
    else {
        return;
    };
    for (index, (x, y)) in xs.iter_mut().zip(ys.iter_mut()).enumerate() {
        let t = (index + 1) as f32 * step;
        *x = p0.x + t * (a.0 + t * (b.0 + t * c.0));
        *y = p0.y + t * (a.1 + t * (b.1 + t * c.1));
    }
    *last_x = p3.x;
    *last_y = p3.y;
}

fn reset(column: &mut Vec<f32>, len: usize) {
    column.clear();
    column.resize(len, 0.0);
}

fn pad(column: &mut [f32], len: usize) {
    let last = len.checked_sub(1).map_or(0.0, |at| column[at]);
    column[len..].fill(last);
}
