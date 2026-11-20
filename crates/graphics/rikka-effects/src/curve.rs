use rikka_image_edit::Spline;
use serde::{Deserialize, Serialize};

pub(crate) const GAP: f32 = 4.0 / 255.0;
pub(crate) const MAX_POINTS: usize = 16;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<(f32, f32)>", into = "Vec<(f32, f32)>")]
pub struct Curve {
    points: Vec<(f32, f32)>,
}

impl From<Vec<(f32, f32)>> for Curve {
    fn from(points: Vec<(f32, f32)>) -> Curve {
        let mut thinned: Vec<(f32, f32)> = Vec::new();
        for point in Curve::from_points(
            points
                .into_iter()
                .filter(|(x, y)| x.is_finite() && y.is_finite()),
        )
        .points
        {
            if thinned.last().is_none_or(|last| point.0 - last.0 >= GAP) {
                thinned.push(point);
            }
        }
        thinned.truncate(MAX_POINTS);
        match thinned.len() >= 2 {
            true => Curve { points: thinned },
            false => Curve::default(),
        }
    }
}

impl From<Curve> for Vec<(f32, f32)> {
    fn from(curve: Curve) -> Vec<(f32, f32)> {
        curve.points
    }
}

impl Default for Curve {
    fn default() -> Curve {
        Curve::from_points([(0.0, 0.0), (1.0, 1.0)])
    }
}

impl Curve {
    pub fn from_points(points: impl IntoIterator<Item = (f32, f32)>) -> Curve {
        let mut points: Vec<(f32, f32)> = points
            .into_iter()
            .map(|(x, y)| (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)))
            .collect();
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        Curve { points }
    }

    pub fn points(&self) -> &[(f32, f32)] {
        &self.points
    }

    pub fn is_identity(&self) -> bool {
        self.points == [(0.0, 0.0), (1.0, 1.0)]
    }

    pub fn insertion(points: &[(f32, f32)], x: f32) -> Option<usize> {
        let (first, last) = (points.first()?.0, points.last()?.0);
        let crowded = points.iter().any(|point| (point.0 - x).abs() < GAP);
        let inside = x > first + GAP && x < last - GAP;
        (points.len() < MAX_POINTS && inside && !crowded)
            .then(|| points.partition_point(|point| point.0 < x))
    }

    pub fn insert(&mut self, x: f32, y: f32) -> Option<usize> {
        let at = Curve::insertion(&self.points, x)?;
        self.points.insert(at, (x, y.clamp(0.0, 1.0)));
        Some(at)
    }

    pub fn moved(&mut self, index: usize, x: f32, y: f32) {
        let last = self.points.len().saturating_sub(1);
        if index > last {
            return;
        }
        let lower = match index {
            0 => 0.0,
            _ => self.points[index - 1].0 + GAP,
        };
        let upper = match index == last {
            true => 1.0,
            false => self.points[index + 1].0 - GAP,
        };
        let x = match lower <= upper {
            true => x.clamp(lower, upper),
            false => self.points[index].0,
        };
        self.points[index] = (x, y.clamp(0.0, 1.0));
    }

    pub fn removable(points: &[(f32, f32)], index: usize) -> bool {
        index > 0 && index + 1 < points.len()
    }

    pub fn remove(&mut self, index: usize) -> bool {
        let removable = Curve::removable(&self.points, index);
        if removable {
            self.points.remove(index);
        }
        removable
    }
}

pub fn spline_of(points: &[(f32, f32)]) -> Spline {
    Spline::from_points(points.iter().map(|&(x, y)| (f64::from(x), f64::from(y))))
}
