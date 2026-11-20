use rikka_image_edit::FillRule;

use crate::area::Area;
use crate::edit::SelectionAction;
use crate::region::{Builder, Region, Span};

const SAMPLES: u32 = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    outline: Outline,
    anti_aliased: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Outline {
    Rectangle {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    },
    Ellipse {
        center: (f32, f32),
        radii: (f32, f32),
    },
    Polygon {
        subpaths: Vec<Vec<(f32, f32)>>,
        rule: FillRule,
        freehand: bool,
    },
}

impl Shape {
    #[must_use]
    pub fn rectangle(left: f32, top: f32, width: f32, height: f32) -> Shape {
        let (left, right) = ordered(left, left + width);
        let (top, bottom) = ordered(top, top + height);
        Shape::from_outline(Outline::Rectangle {
            left,
            top,
            right,
            bottom,
        })
    }

    #[must_use]
    pub fn ellipse(left: f32, top: f32, width: f32, height: f32) -> Shape {
        let (left, right) = ordered(left, left + width);
        let (top, bottom) = ordered(top, top + height);
        Shape::from_outline(Outline::Ellipse {
            center: ((left + right) / 2.0, (top + bottom) / 2.0),
            radii: ((right - left) / 2.0, (bottom - top) / 2.0),
        })
    }

    #[must_use]
    pub fn polygon<S, P>(subpaths: S, rule: FillRule) -> Shape
    where
        S: IntoIterator<Item = P>,
        P: IntoIterator<Item = (f32, f32)>,
    {
        Shape::from_outline(Outline::Polygon {
            subpaths: subpaths.into_iter().map(finite).collect(),
            rule,
            freehand: false,
        })
    }

    #[must_use]
    pub fn lasso(trail: impl IntoIterator<Item = (f32, f32)>) -> Shape {
        Shape::from_outline(Outline::Polygon {
            subpaths: vec![finite(trail)],
            rule: FillRule::NonZero,
            freehand: true,
        })
    }

    #[must_use]
    pub fn anti_aliased(mut self, anti_aliased: bool) -> Shape {
        self.anti_aliased = anti_aliased;
        self
    }

    fn from_outline(outline: Outline) -> Shape {
        Shape {
            outline,
            anti_aliased: true,
        }
    }

    #[must_use]
    pub fn region(&self, canvas: Area) -> Region {
        let Some((top, bottom)) = self.outline.rows(canvas) else {
            return Region::default();
        };
        let (left, right) = (canvas.left as f32, canvas.right() as f32);
        let mut scanner = Scanner::new(&self.outline);
        let mut builder = Builder::new(top);
        let mut intervals = Vec::new();
        let mut row = Row::default();
        for y in top..bottom {
            match self.anti_aliased {
                true => {
                    row.clear();
                    for sample in 0..SAMPLES {
                        let at = y as f32 + (sample as f32 + 0.5) / SAMPLES as f32;
                        scanner.intervals(at, &mut intervals);
                        for &(from, to) in &intervals {
                            row.add(from.max(left), to.min(right));
                        }
                    }
                    row.emit(&mut builder);
                }
                false => {
                    scanner.intervals(y as f32 + 0.5, &mut intervals);
                    for &(from, to) in &intervals {
                        let start = (from - 0.5).ceil().clamp(left, right) as u32;
                        let end = (to - 0.5).ceil().clamp(left, right) as u32;
                        builder.push(start, end, Span::FULL);
                    }
                }
            }
            builder.end_row();
        }
        builder.finish()
    }

    pub(crate) fn selecting(&self) -> SelectionAction {
        match &self.outline {
            Outline::Rectangle { .. } => SelectionAction::Rectangle,
            Outline::Ellipse { .. } => SelectionAction::Ellipse,
            Outline::Polygon { freehand: true, .. } => SelectionAction::Lasso,
            Outline::Polygon {
                freehand: false, ..
            } => SelectionAction::Polygon,
        }
    }
}

fn ordered(one: f32, other: f32) -> (f32, f32) {
    match one.is_finite() && other.is_finite() {
        true => (one.min(other), one.max(other)),
        false => (0.0, 0.0),
    }
}

fn finite(points: impl IntoIterator<Item = (f32, f32)>) -> Vec<(f32, f32)> {
    points
        .into_iter()
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .collect()
}

impl Outline {
    fn rows(&self, canvas: Area) -> Option<(u32, u32)> {
        let (top, bottom) = match self {
            Outline::Rectangle { top, bottom, .. } => (*top, *bottom),
            Outline::Ellipse { center, radii } => (center.1 - radii.1, center.1 + radii.1),
            Outline::Polygon { subpaths, .. } => subpaths.iter().flatten().fold(
                (f32::INFINITY, f32::NEG_INFINITY),
                |(top, bottom), point| (top.min(point.1), bottom.max(point.1)),
            ),
        };
        let first = top.floor().max(canvas.top as f32);
        let last = bottom.ceil().min((canvas.top + canvas.height) as f32);
        (first < last).then_some((first as u32, last as u32))
    }
}

struct Edge {
    top: f32,
    bottom: f32,
    x: f32,
    slope: f32,
    winding: i32,
}

enum Scanner {
    Rectangle {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    },
    Ellipse {
        center: (f32, f32),
        radii: (f32, f32),
    },
    Polygon {
        edges: Vec<Edge>,
        next: usize,
        active: Vec<usize>,
        crossings: Vec<(f32, i32)>,
        rule: FillRule,
    },
}

impl Scanner {
    fn new(outline: &Outline) -> Scanner {
        match outline {
            Outline::Rectangle {
                left,
                top,
                right,
                bottom,
            } => Scanner::Rectangle {
                left: *left,
                top: *top,
                right: *right,
                bottom: *bottom,
            },
            Outline::Ellipse { center, radii } => Scanner::Ellipse {
                center: *center,
                radii: *radii,
            },
            Outline::Polygon { subpaths, rule, .. } => {
                let mut edges: Vec<Edge> = subpaths
                    .iter()
                    .filter(|points| points.len() > 2)
                    .flat_map(|points| {
                        points
                            .iter()
                            .zip(points.iter().cycle().skip(1))
                            .filter_map(|(from, to)| edge(*from, *to))
                    })
                    .collect();
                edges.sort_unstable_by(|one, other| one.top.total_cmp(&other.top));
                Scanner::Polygon {
                    edges,
                    next: 0,
                    active: Vec::new(),
                    crossings: Vec::new(),
                    rule: *rule,
                }
            }
        }
    }

    fn intervals(&mut self, y: f32, out: &mut Vec<(f32, f32)>) {
        out.clear();
        match self {
            Scanner::Rectangle {
                left,
                top,
                right,
                bottom,
            } => {
                if *top <= y && y < *bottom && left < right {
                    out.push((*left, *right));
                }
            }
            Scanner::Ellipse { center, radii } => {
                if radii.0 <= 0.0 || radii.1 <= 0.0 {
                    return;
                }
                let along = (y - center.1) / radii.1;
                if along.abs() < 1.0 {
                    let half = radii.0 * (1.0 - along * along).sqrt();
                    out.push((center.0 - half, center.0 + half));
                }
            }
            Scanner::Polygon {
                edges,
                next,
                active,
                crossings,
                rule,
                ..
            } => {
                while *next < edges.len() && edges[*next].top <= y {
                    active.push(*next);
                    *next += 1;
                }
                active.retain(|index| edges[*index].bottom > y);
                crossings.clear();
                crossings.extend(
                    active
                        .iter()
                        .map(|index| &edges[*index])
                        .filter(|edge| edge.top <= y)
                        .map(|edge| (edge.x + (y - edge.top) * edge.slope, edge.winding)),
                );
                crossings.sort_unstable_by(|one, other| one.0.total_cmp(&other.0));
                let mut winding = 0;
                let mut entered_at = None;
                for &(x, turn) in crossings.iter() {
                    winding += turn;
                    let inside = match rule {
                        FillRule::NonZero => winding != 0,
                        FillRule::EvenOdd => winding % 2 != 0,
                    };
                    match (inside, entered_at) {
                        (true, None) => entered_at = Some(x),
                        (false, Some(from)) => {
                            if from < x {
                                out.push((from, x));
                            }
                            entered_at = None;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn edge(from: (f32, f32), to: (f32, f32)) -> Option<Edge> {
    if from.1 == to.1 {
        return None;
    }
    let (upper, lower, winding) = match from.1 < to.1 {
        true => (from, to, 1),
        false => (to, from, -1),
    };
    Some(Edge {
        top: upper.1,
        bottom: lower.1,
        x: upper.0,
        slope: (lower.0 - upper.0) / (lower.1 - upper.1),
        winding,
    })
}

#[derive(Default)]
struct Row {
    steps: Vec<(u32, i32)>,
    parts: Vec<(u32, f32)>,
}

impl Row {
    fn clear(&mut self) {
        self.steps.clear();
        self.parts.clear();
    }

    fn add(&mut self, from: f32, to: f32) {
        if from >= to {
            return;
        }
        let (first, last) = (from.floor(), to.floor());
        if first == last {
            self.parts.push((first as u32, to - from));
            return;
        }
        self.parts.push((first as u32, first + 1.0 - from));
        if first + 1.0 < last {
            self.steps.push((first as u32 + 1, 1));
            self.steps.push((last as u32, -1));
        }
        if to > last {
            self.parts.push((last as u32, to - last));
        }
    }

    fn emit(&mut self, builder: &mut Builder) {
        self.steps.sort_unstable_by_key(|step| step.0);
        self.parts.sort_unstable_by_key(|part| part.0);
        let mut parts: Vec<(u32, f32)> = Vec::with_capacity(self.parts.len());
        for &(x, amount) in &self.parts {
            match parts.last_mut() {
                Some(last) if last.0 == x => last.1 += amount,
                _ => parts.push((x, amount)),
            }
        }
        let first = match (self.steps.first(), parts.first()) {
            (None, None) => return,
            (Some(step), None) => step.0,
            (None, Some(part)) => part.0,
            (Some(step), Some(part)) => step.0.min(part.0),
        };
        let (mut step, mut part, mut count, mut x) = (0, 0, 0i32, first);
        let level = |amount: f32| (amount / SAMPLES as f32).clamp(0.0, 1.0);
        loop {
            while step < self.steps.len() && self.steps[step].0 <= x {
                count += self.steps[step].1;
                step += 1;
            }
            if part < parts.len() && parts[part].0 == x {
                builder.push(x, x + 1, quantized(level(count as f32 + parts[part].1)));
                part += 1;
                x += 1;
                continue;
            }
            let next = match (self.steps.get(step), parts.get(part)) {
                (None, None) => break,
                (Some(step), None) => step.0,
                (None, Some(part)) => part.0,
                (Some(step), Some(part)) => step.0.min(part.0),
            };
            builder.push(x, next, quantized(level(count as f32)));
            x = next;
        }
    }
}

pub(crate) fn quantized(level: f32) -> u8 {
    (level * f32::from(Span::FULL)).round() as u8
}
