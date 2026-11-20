use rikka_types::Point;

use crate::flatten::flatness::Flatness;
use crate::path::{Path, Verb};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Vertex {
    pub(crate) at: Point,
    pub(crate) smooth: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Polyline {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) closed: bool,
}

impl Polyline {
    fn begin(&mut self, at: Point) {
        self.vertices.clear();
        self.closed = false;
        self.vertices.push(Vertex { at, smooth: false });
    }

    fn reach(&mut self, at: Point, smooth: bool) {
        match self.vertices.last_mut() {
            Some(last) if last.at == at => last.smooth &= smooth,
            _ => self.vertices.push(Vertex { at, smooth }),
        }
    }

    fn finish(&mut self, closed: bool) {
        self.closed = closed;
        if closed
            && self.vertices.len() > 1
            && self.vertices.first().map(|first| first.at)
                == self.vertices.last().map(|last| last.at)
        {
            self.vertices.pop();
        }
        if let Some(first) = self.vertices.first_mut() {
            first.smooth = false;
        }
    }
}

pub(crate) fn each_polyline(
    path: &Path,
    tolerance: f32,
    line: &mut Polyline,
    mut emit: impl FnMut(&Polyline),
) {
    let flatness = Flatness::new(tolerance);
    let points = path.points();
    let mut index = 0;
    let mut open = false;
    for &verb in path.verbs() {
        let from = points.get(index.max(1) - 1).copied().unwrap_or_default();
        match verb {
            Verb::Move => {
                if open {
                    line.finish(false);
                    emit(line);
                }
                line.begin(points[index]);
                open = true;
                index += 1;
            }
            Verb::Line => {
                line.reach(points[index], false);
                index += 1;
            }
            Verb::Quad => {
                let (control, to) = (points[index], points[index + 1]);
                let steps = flatness.quad_steps(from, control, to);
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    let u = 1.0 - t;
                    let at = match step == steps {
                        true => to,
                        false => from * (u * u) + control * (2.0 * u * t) + to * (t * t),
                    };
                    line.reach(at, step != steps);
                }
                index += 2;
            }
            Verb::Cubic => {
                let (first, second, to) = (points[index], points[index + 1], points[index + 2]);
                let steps = flatness.cubic_steps(from, first, second, to);
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    let u = 1.0 - t;
                    let at = match step == steps {
                        true => to,
                        false => {
                            from * (u * u * u)
                                + first * (3.0 * u * u * t)
                                + second * (3.0 * u * t * t)
                                + to * (t * t * t)
                        }
                    };
                    line.reach(at, step != steps);
                }
                index += 3;
            }
            Verb::Close => {
                if open {
                    line.finish(true);
                    emit(line);
                    open = false;
                }
            }
        }
    }
    if open {
        line.finish(false);
        emit(line);
    }
}
