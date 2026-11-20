use rikka_figure::LetterWipe;
use rikka_types::{Point, Rect};

use super::homography::Homography;
use super::shot::{NEAR, Projection};
use crate::animation_presets::Wipe;

type Edge = [f64; 3];
type Spot = [f64; 2];

impl Projection {
    #[must_use]
    pub fn clipped(&self, polygon: &[Point], within: Rect) -> Vec<Point> {
        let mut clipped: Vec<Spot> = polygon
            .iter()
            .map(|point| [f64::from(point.x), f64::from(point.y)])
            .collect();
        for edge in self
            .near_edges()
            .into_iter()
            .chain(self.frame_edges(within))
        {
            if clipped.is_empty() {
                break;
            }
            clipped = cut(&clipped, edge);
        }
        clipped
            .into_iter()
            .map(|[u, v]| Point::new(u as f32, v as f32))
            .collect()
    }

    #[must_use]
    pub fn crosses_near_plane(&self, polygon: &[Point]) -> bool {
        let edges = self.near_edges();
        polygon.iter().any(|point| {
            let spot = [f64::from(point.x), f64::from(point.y)];
            edges.iter().any(|edge| side(*edge, spot) <= 0.0)
        })
    }

    #[must_use]
    pub fn stretch(&self, local: Point) -> Option<f32> {
        self.map(local)?;
        let [across, down, depth] = self.homography().rows();
        let spot = [f64::from(local.x), f64::from(local.y)];
        let w = side(depth, spot);
        let x = side(across, spot) / w;
        let y = side(down, spot) / w;
        let along_u = [(across[0] - x * depth[0]) / w, (down[0] - y * depth[0]) / w];
        let along_v = [(across[1] - x * depth[1]) / w, (down[1] - y * depth[1]) / w];
        let total =
            along_u[0].powi(2) + along_u[1].powi(2) + along_v[0].powi(2) + along_v[1].powi(2);
        let area = along_u[0] * along_v[1] - along_u[1] * along_v[0];
        let spread = (total * total - 4.0 * area * area).max(0.0).sqrt();
        let largest = ((total + spread) / 2.0).sqrt();
        largest.is_finite().then_some(largest as f32)
    }

    fn near_edges(&self) -> Vec<Edge> {
        let mut before = Homography::IDENTITY;
        let mut edges = Vec::with_capacity(self.stages().len());
        for stage in self.stages() {
            let after = before.then(*stage);
            let (now, then) = (after.rows()[2], before.rows()[2]);
            edges.push(std::array::from_fn(|index| now[index] - NEAR * then[index]));
            before = after;
        }
        edges
    }

    fn frame_edges(&self, within: Rect) -> [Edge; 4] {
        let [across, down, depth] = self.homography().rows();
        let bounded = |row: [f64; 3], limit: f32, sign: f64| -> Edge {
            std::array::from_fn(|index| sign * (row[index] - f64::from(limit) * depth[index]))
        };
        [
            bounded(across, within.min_x(), 1.0),
            bounded(across, within.max_x(), -1.0),
            bounded(down, within.min_y(), 1.0),
            bounded(down, within.max_y(), -1.0),
        ]
    }
}

impl Wipe {
    #[must_use]
    pub fn clip_to(&self, polygon: &[Point], across: Rect) -> Vec<Point> {
        LetterWipe {
            from: self.direction.vector(),
            progress: self.progress,
        }
        .cut(polygon, across)
    }
}

fn side(edge: Edge, spot: Spot) -> f64 {
    edge[0] * spot[0] + edge[1] * spot[1] + edge[2]
}

fn cut(polygon: &[Spot], edge: Edge) -> Vec<Spot> {
    let mut clipped = Vec::with_capacity(polygon.len() + 2);
    for (index, &current) in polygon.iter().enumerate() {
        let previous = polygon[(index + polygon.len() - 1) % polygon.len()];
        let (was, is) = (side(edge, previous), side(edge, current));
        if (was >= 0.0) != (is >= 0.0) {
            let along = was / (was - is);
            clipped.push([
                previous[0] + (current[0] - previous[0]) * along,
                previous[1] + (current[1] - previous[1]) * along,
            ]);
        }
        if is >= 0.0 {
            clipped.push(current);
        }
    }
    clipped
}
