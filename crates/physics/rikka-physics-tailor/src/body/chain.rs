use glam::{DVec3, Vec2, Vec3};

use super::circle::enclosing;
use super::cone::Cone;

const PIECES: usize = 3;
const TOLERANCE: f32 = 0.002;
const THINNEST: f32 = 0.001;
const END_SHARE: f32 = 1.0 / 3.0;
const INSET_STEPS: u32 = 48;
const STEEPEST_TURN: f32 = 0.5;

pub(crate) fn chains(points: &[Vec3], direction: Vec3) -> Vec<Vec<Cone>> {
    let Some(axis) = Axis::from_direction(points, direction) else {
        return Vec::new();
    };
    let hull = upper_hull(points.iter().map(|point| axis.place(*point)));
    let (Some(first), Some(last)) = (hull.first().copied(), hull.last().copied()) else {
        return Vec::new();
    };
    let half = (last.x - first.x) * 0.5;
    let set_in = inset(&hull, half);
    let mirrored: Vec<Vec2> = hull.iter().rev().map(|v| Vec2::new(-v.x, v.y)).collect();
    let strand_node = inset(&mirrored, half);
    let set_out = Vec2::new(-strand_node.x, strand_node.y);
    let mut variants: Vec<Vec<Cone>> = Vec::with_capacity(4);
    for start in [first, set_in] {
        for end in [last, set_out] {
            let between = hull
                .iter()
                .copied()
                .filter(|v| v.x > start.x && v.x < end.x);
            let outline = upper_hull(std::iter::once(start).chain(between).chain([end]));
            let strand_nodes = simplified(&outline);
            let cones = match strand_nodes.as_slice() {
                [only] => vec![axis.cone(*only, *only)],
                _ => strand_nodes
                    .windows(2)
                    .map(|pair| axis.cone(pair[0], pair[1]))
                    .collect(),
            };
            if !variants.contains(&cones) {
                variants.push(cones);
            }
        }
    }
    variants
}

struct Axis {
    origin: Vec3,
    direction: Vec3,
}

impl Axis {
    fn from_direction(points: &[Vec3], direction: Vec3) -> Option<Self> {
        if points.is_empty() {
            return None;
        }
        let sum: DVec3 = points.iter().map(|p| p.as_dvec3()).sum();
        let centroid = (sum / points.len() as f64).as_vec3();
        let (u, v) = direction.any_orthonormal_pair();
        let along: Vec<f32> = points
            .iter()
            .map(|p| (*p - centroid).dot(direction))
            .collect();
        let low = along.iter().copied().fold(f32::MAX, f32::min);
        let high = along.iter().copied().fold(f32::MIN, f32::max);
        let reach = (high - low) * END_SHARE;
        let flat = |keep: &dyn Fn(f32) -> bool| -> Vec<(f32, Vec2)> {
            points
                .iter()
                .zip(&along)
                .filter(|(_, t)| keep(**t))
                .map(|(point, t)| {
                    let offset = *point - centroid;
                    (*t, Vec2::new(offset.dot(u), offset.dot(v)))
                })
                .collect()
        };
        let centre = |samples: Vec<(f32, Vec2)>| -> Option<Vec3> {
            let planar: Vec<Vec2> = samples.iter().map(|(_, at)| *at).collect();
            let circle = enclosing(&planar)?;
            let depth = samples.iter().map(|(t, _)| t).sum::<f32>() / samples.len() as f32;
            Some(centroid + direction * depth + u * circle.centre.x + v * circle.centre.y)
        };
        let ends = (
            centre(flat(&|t| t <= low + reach)),
            centre(flat(&|t| t >= high - reach)),
        );
        let refined = match ends {
            (Some(first), Some(last)) => {
                let turned = (last - first).normalize_or_zero();
                (turned.dot(direction) > STEEPEST_TURN).then_some(Self {
                    origin: first,
                    direction: turned,
                })
            }
            _ => None,
        };
        refined.or_else(|| centre(flat(&|_| true)).map(|origin| Self { origin, direction }))
    }

    fn place(&self, point: Vec3) -> Vec2 {
        let offset = point - self.origin;
        let t = offset.dot(self.direction);
        Vec2::new(t, (offset - self.direction * t).length())
    }

    fn cone(&self, a: Vec2, b: Vec2) -> Cone {
        Cone {
            a: self.origin + self.direction * a.x,
            b: self.origin + self.direction * b.x,
            radius_a: a.y.max(THINNEST),
            radius_b: b.y.max(THINNEST),
        }
    }
}
fn upper_hull(points: impl Iterator<Item = Vec2>) -> Vec<Vec2> {
    let mut sorted: Vec<Vec2> = points.filter(|p| p.is_finite()).collect();
    sorted.sort_by(|a, b| a.x.total_cmp(&b.x).then(b.y.total_cmp(&a.y)));
    sorted.dedup_by(|later, earlier| later.x == earlier.x);
    let mut hull: Vec<Vec2> = Vec::with_capacity(sorted.len().min(64));
    for point in sorted {
        while let [.., before, last] = hull.as_slice()
            && (*last - *before).perp_dot(point - *before) >= 0.0
        {
            hull.pop();
        }
        hull.push(point);
    }
    hull
}

fn height(chain: &[Vec2], t: f32) -> f32 {
    let after = chain.partition_point(|v| v.x < t);
    match (after.checked_sub(1).map(|i| chain[i]), chain.get(after)) {
        (Some(a), Some(b)) => a.y + (b.y - a.y) * (t - a.x) / (b.x - a.x),
        (None, Some(b)) => b.y,
        (Some(a), None) => a.y,
        (None, None) => 0.0,
    }
}

fn inset(hull: &[Vec2], half: f32) -> Vec2 {
    let start = hull[0].x;
    let limit = start + half;
    let past_start = |t: f32| {
        hull.iter()
            .filter(|v| v.x >= t)
            .map(|v| v.y - (v.x - start))
            .fold(0.0f32, f32::max)
    };
    let uniform = (0..=INSET_STEPS).map(|step| start + half * step as f32 / INSET_STEPS as f32);
    let corners = hull.iter().map(|v| v.x).filter(|t| *t <= limit);
    let mut best = (f32::MAX, hull[0]);
    for t in uniform.chain(corners) {
        let side = height(hull, t);
        let reach = hull
            .iter()
            .take_while(|v| v.x <= t)
            .map(|v| Vec2::new(t - v.x, v.y).length())
            .fold(side, f32::max);
        let cost = (reach - (t - start)).max(reach - side).max(past_start(t));
        if cost < best.0 {
            best = (cost, Vec2::new(t, reach));
        }
    }
    best.1
}

#[derive(Clone, Copy, Debug)]
struct Line {
    slope: f32,
    offset: f32,
}

impl Line {
    fn from_points(a: Vec2, b: Vec2) -> Self {
        let slope = (b.y - a.y) / (b.x - a.x);
        Self {
            slope,
            offset: a.y - slope * a.x,
        }
    }

    fn at(&self, t: f32) -> f32 {
        self.slope * t + self.offset
    }

    fn meet(&self, other: &Self) -> f32 {
        (other.offset - self.offset) / (self.slope - other.slope)
    }
}

fn simplified(outline: &[Vec2]) -> Vec<Vec2> {
    let (Some(first), Some(last)) = (outline.first().copied(), outline.last().copied()) else {
        return Vec::new();
    };
    if outline.len() <= 2 {
        return outline.to_vec();
    }
    let mut lines: Vec<Line> = outline
        .windows(2)
        .map(|pair| Line::from_points(pair[0], pair[1]))
        .collect();
    let cost = |lines: &[Line], at: usize| match (
        at.checked_sub(1).map(|i| lines[i]),
        lines.get(at + 1),
    ) {
        (None, Some(next)) => next.at(first.x) - first.y,
        (Some(previous), None) => previous.at(last.x) - last.y,
        (Some(previous), Some(next)) => {
            let t = previous.meet(next);
            previous.at(t) - height(outline, t)
        }
        (None, None) => f32::INFINITY,
    };
    while lines.len() > 1 {
        let Some((at, gap)) = (0..lines.len())
            .map(|at| (at, cost(&lines, at)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
        else {
            break;
        };
        if lines.len() <= PIECES && gap > TOLERANCE {
            break;
        }
        lines.remove(at);
    }
    let envelope = |t: f32| lines.iter().map(|line| line.at(t)).fold(f32::MAX, f32::min);
    let mut strand_nodes = vec![Vec2::new(first.x, envelope(first.x))];
    for pair in lines.windows(2) {
        let t = pair[0].meet(&pair[1]).clamp(first.x, last.x);
        strand_nodes.push(Vec2::new(t, envelope(t)));
    }
    strand_nodes.push(Vec2::new(last.x, envelope(last.x)));
    strand_nodes
}
