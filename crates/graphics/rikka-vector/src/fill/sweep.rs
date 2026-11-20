use crate::fill::rasterizer::Area;
use crate::fill::rule::FillRule;
use crate::flatten::polylines::Polylines;

const SAMPLES: usize = 32;
const WEIGHT: f32 = 1.0 / SAMPLES as f32;

#[derive(Clone, Copy, Debug)]
struct Edge {
    top: f32,
    bottom: f32,
    x: f32,
    slope: f32,
    winding: i32,
    crossing: f32,
}

#[derive(Debug, Default)]
pub(crate) struct Sweep {
    edges: Vec<Edge>,
    active: Vec<Edge>,
    steps: Vec<f32>,
}

impl Sweep {
    pub(crate) fn coverage(
        &mut self,
        lines: &Polylines,
        rule: FillRule,
        area: Area,
        out: &mut [f32],
    ) {
        let (width, height) = (area.width as usize, area.height as usize);
        self.collect(lines, area);
        self.active.clear();
        self.steps.clear();
        self.steps.resize(width + 2, 0.0);
        let mut pending = self.edges.iter().peekable();
        for (row, out) in out.chunks_exact_mut(width).take(height).enumerate() {
            for sample in 0..SAMPLES {
                let y = row as f32 + (sample as f32 + 0.5) * WEIGHT;
                while let Some(edge) = pending.next_if(|edge| edge.top <= y) {
                    self.active.push(*edge);
                }
                self.active.retain(|edge| edge.bottom > y);
                for edge in &mut self.active {
                    edge.crossing = edge.x + (y - edge.top) * edge.slope;
                }
                self.active
                    .sort_unstable_by(|one, other| one.crossing.total_cmp(&other.crossing));
                let (mut winding, mut entered) = (0, 0.0);
                for edge in &self.active {
                    let was_inside = inside(rule, winding);
                    winding += edge.winding;
                    match (was_inside, inside(rule, winding)) {
                        (false, true) => entered = edge.crossing,
                        (true, false) => {
                            step(&mut self.steps, entered, WEIGHT, width);
                            step(&mut self.steps, edge.crossing, -WEIGHT, width);
                        }
                        _ => {}
                    }
                }
            }
            let mut sum = 0.0f32;
            for (target, step) in out.iter_mut().zip(&mut self.steps) {
                sum += *step;
                *target = sum.clamp(0.0, 1.0);
            }
            self.steps.fill(0.0);
        }
    }

    pub(crate) fn aliased(
        &mut self,
        lines: &Polylines,
        rule: FillRule,
        area: Area,
        out: &mut [f32],
    ) {
        let (width, height) = (area.width as usize, area.height as usize);
        self.collect(lines, area);
        self.active.clear();
        self.steps.clear();
        self.steps.resize(width + 2, 0.0);
        let mut pending = self.edges.iter().peekable();
        for (row, out) in out.chunks_exact_mut(width).take(height).enumerate() {
            let y = row as f32 + 0.5;
            while let Some(edge) = pending.next_if(|edge| edge.top <= y) {
                self.active.push(*edge);
            }
            self.active.retain(|edge| edge.bottom > y);
            for edge in &mut self.active {
                edge.crossing = edge.x + (y - edge.top) * edge.slope;
            }
            self.active
                .sort_unstable_by(|one, other| one.crossing.total_cmp(&other.crossing));
            let (mut winding, mut entered) = (0, 0.0);
            for edge in &self.active {
                let was_inside = inside(rule, winding);
                winding += edge.winding;
                match (was_inside, inside(rule, winding)) {
                    (false, true) => entered = edge.crossing,
                    (true, false) => {
                        let first = center_column(entered, width);
                        let end = center_column(edge.crossing, width);
                        if first < end {
                            self.steps[first] += 1.0;
                            self.steps[end] -= 1.0;
                        }
                    }
                    _ => {}
                }
            }
            let mut sum = 0.0f32;
            for (target, step) in out.iter_mut().zip(&mut self.steps) {
                sum += *step;
                *target = sum.clamp(0.0, 1.0);
            }
            self.steps.fill(0.0);
        }
    }

    fn collect(&mut self, lines: &Polylines, area: Area) {
        let (left, top) = (area.left as f32, area.top as f32);
        let height = area.height as f32;
        self.edges.clear();
        for contour in lines.contours() {
            let (xs, ys) = (&lines.xs[contour.clone()], &lines.ys[contour]);
            for (x, y) in xs.windows(2).zip(ys.windows(2)) {
                let (from, to, winding) = match y[0] < y[1] {
                    true => ((x[0], y[0]), (x[1], y[1]), 1),
                    false => ((x[1], y[1]), (x[0], y[0]), -1),
                };
                let edge = Edge {
                    top: from.1 - top,
                    bottom: to.1 - top,
                    x: from.0 - left,
                    slope: (to.0 - from.0) / (to.1 - from.1),
                    winding,
                    crossing: 0.0,
                };
                if edge.top < edge.bottom && edge.bottom > 0.0 && edge.top < height {
                    self.edges.push(edge);
                }
            }
        }
        self.edges
            .sort_unstable_by(|one, other| one.top.total_cmp(&other.top));
    }
}

#[inline]
fn inside(rule: FillRule, winding: i32) -> bool {
    match rule {
        FillRule::NonZero => winding != 0,
        FillRule::EvenOdd => winding & 1 != 0,
    }
}

#[inline]
fn step(steps: &mut [f32], x: f32, weight: f32, width: usize) {
    let x = x.clamp(0.0, width as f32);
    let column = x as usize;
    let fraction = x - column as f32;
    steps[column] += weight * (1.0 - fraction);
    steps[column + 1] += weight * fraction;
}

#[inline]
fn center_column(x: f32, width: usize) -> usize {
    (x - 0.5).ceil().clamp(0.0, width as f32) as usize
}
