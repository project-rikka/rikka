use std::f32::consts::{PI, TAU};

use rikka_types::Point;

use crate::path::Path;
use crate::stroke::dash::each_dash;
use crate::stroke::polyline::{Polyline, Vertex, each_polyline};
use crate::stroke::style::{Cap, Join, Stroke};

const PARALLEL: f32 = 1.0e-6;
const MIN_ARC_PIECES: usize = 8;
const MAX_ARC_PIECES: usize = 512;

#[derive(Debug, Default)]
pub struct Stroker {
    line: Polyline,
    piece: Polyline,
    held: Vec<Vertex>,
    spans: Vec<Span>,
}

#[derive(Clone, Copy, Debug)]
struct Span {
    direction: Point,
    length: f32,
}

impl Span {
    fn reversed(self) -> Span {
        Span {
            direction: -self.direction,
            length: self.length,
        }
    }
}

struct Pen<'p> {
    out: &'p mut Path,
    half: f32,
    join: Join,
    cap: Cap,
    miter_limit: f32,
    turn_pieces: f32,
    started: bool,
}

impl Stroker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn outline(&mut self, path: &Path, stroke: &Stroke<'_>, tolerance: f32, out: &mut Path) {
        out.clear();
        let half = stroke.width * 0.5;
        if half <= 0.0 || !half.is_finite() || tolerance <= 0.0 || tolerance.is_nan() {
            return;
        }
        let Self {
            line,
            piece,
            held,
            spans,
        } = self;
        let mut pen = Pen {
            out,
            half,
            join: stroke.join,
            cap: stroke.cap,
            miter_limit: stroke.miter_limit.max(1.0),
            turn_pieces: full_turn_pieces(half, tolerance),
            started: false,
        };
        let dashed = stroke.is_dashed();
        each_polyline(path, tolerance, line, |line| match dashed {
            true => each_dash(
                line,
                stroke.dashes,
                stroke.dash_offset,
                piece,
                held,
                |dash| {
                    pen.polyline(dash, spans);
                },
            ),
            false => pen.polyline(line, spans),
        });
    }
}

fn full_turn_pieces(radius: f32, tolerance: f32) -> f32 {
    let step = (1.0 - tolerance / radius.max(tolerance))
        .clamp(-1.0, 1.0)
        .acos()
        * 2.0;
    let pieces = match step > 0.0 {
        true => (TAU / step).ceil() as usize,
        false => MAX_ARC_PIECES,
    };
    pieces.clamp(MIN_ARC_PIECES, MAX_ARC_PIECES) as f32
}

fn left(direction: Point) -> Point {
    Point::new(-direction.y, direction.x)
}

fn cross(a: Point, b: Point) -> f32 {
    a.x * b.y - a.y * b.x
}

fn dot(a: Point, b: Point) -> f32 {
    a.x * b.x + a.y * b.y
}

impl Pen<'_> {
    fn polyline(&mut self, line: &Polyline, spans: &mut Vec<Span>) {
        let vertices = &line.vertices;
        match vertices.len() {
            0 => {}
            1 => self.dot(vertices[0].at, Point::new(1.0, 0.0)),
            _ => {
                let count = vertices.len();
                let segments = match line.closed {
                    true => count,
                    false => count - 1,
                };
                spans.clear();
                spans.extend((0..segments).map(|at| {
                    let delta = vertices[(at + 1) % count].at - vertices[at].at;
                    let length = delta.x.hypot(delta.y);
                    Span {
                        direction: delta * (1.0 / length),
                        length,
                    }
                }));
                match line.closed {
                    true => {
                        self.ring(vertices, spans, false);
                        self.ring(vertices, spans, true);
                    }
                    false => self.strip(vertices, spans),
                }
            }
        }
    }

    fn dot(&mut self, at: Point, direction: Point) {
        let across = left(direction) * self.half;
        match self.cap {
            Cap::Butt => {}
            Cap::Round => {
                self.begin(at + across);
                self.arc(at, across, TAU, false);
                self.out.close();
            }
            Cap::Square => {
                let along = direction * self.half;
                self.begin(at + across - along);
                self.reach(at + across + along);
                self.reach(at - across + along);
                self.reach(at - across - along);
                self.out.close();
            }
        }
    }

    fn strip(&mut self, vertices: &[Vertex], spans: &[Span]) {
        let last = vertices.len() - 1;
        self.begin(vertices[0].at + left(spans[0].direction) * self.half);
        for at in 1..last {
            self.join(vertices[at], spans[at - 1], spans[at]);
        }
        self.cap(vertices[last].at, spans[last - 1].direction);
        for at in (1..last).rev() {
            self.join(vertices[at], spans[at].reversed(), spans[at - 1].reversed());
        }
        self.cap(vertices[0].at, -spans[0].direction);
        self.out.close();
    }

    fn ring(&mut self, vertices: &[Vertex], spans: &[Span], reversed: bool) {
        let count = vertices.len();
        self.started = false;
        for step in 0..count {
            let (at, incoming, outgoing) = match reversed {
                false => (step, spans[(step + count - 1) % count], spans[step]),
                true => {
                    let at = count - 1 - step;
                    (
                        at,
                        spans[at].reversed(),
                        spans[(at + count - 1) % count].reversed(),
                    )
                }
            };
            self.join(vertices[at], incoming, outgoing);
        }
        self.out.close();
    }

    fn join(&mut self, vertex: Vertex, before: Span, after: Span) {
        let (incoming, outgoing) = (before.direction, after.direction);
        let pivot = vertex.at;
        let from = left(incoming) * self.half;
        let to = left(outgoing) * self.half;
        let turn = cross(incoming, outgoing);
        let along = dot(incoming, outgoing);
        if turn.abs() < PARALLEL && along > 0.0 {
            self.reach(pivot + to);
            return;
        }
        if turn > 0.0 {
            let inset = self.half * turn / (1.0 + along);
            if along > -1.0 && inset <= before.length.min(after.length) * 0.5 {
                self.reach(pivot + (from + to) * (1.0 / (1.0 + along)));
                return;
            }
            self.reach(pivot + from);
            self.reach(pivot);
            self.reach(pivot + to);
            return;
        }
        self.reach(pivot + from);
        let join = match vertex.smooth {
            true => Join::Round,
            false => self.join,
        };
        match join {
            Join::Bevel => {}
            Join::Round => {
                let angle = turn.abs().atan2(along);
                self.arc(pivot, from, angle, true);
            }
            Join::Miter => {
                let cosine_half = ((1.0 + along) * 0.5).max(0.0).sqrt();
                if cosine_half > 0.0 && 1.0 / cosine_half <= self.miter_limit {
                    self.reach(pivot + (from + to) * (1.0 / (1.0 + along)));
                }
            }
        }
        self.reach(pivot + to);
    }

    fn cap(&mut self, at: Point, outward: Point) {
        let across = left(outward) * self.half;
        self.reach(at + across);
        match self.cap {
            Cap::Butt => {}
            Cap::Round => self.arc(at, across, PI, true),
            Cap::Square => {
                let along = outward * self.half;
                self.reach(at + across + along);
                self.reach(at - across + along);
            }
        }
        self.reach(at - across);
    }

    fn arc(&mut self, center: Point, from: Point, angle: f32, inner_only: bool) {
        let pieces = ((angle / TAU * self.turn_pieces).ceil() as usize).max(1);
        let step = -angle / pieces as f32;
        let last = match inner_only {
            true => pieces - 1,
            false => pieces,
        };
        for piece in 1..=last {
            let (sin, cos) = (step * piece as f32).sin_cos();
            self.reach(
                center + Point::new(from.x * cos - from.y * sin, from.x * sin + from.y * cos),
            );
        }
    }

    fn begin(&mut self, at: Point) {
        self.out.move_to(at);
        self.started = true;
    }

    fn reach(&mut self, at: Point) {
        match self.started {
            true => self.out.line_to(at),
            false => self.begin(at),
        }
    }
}
