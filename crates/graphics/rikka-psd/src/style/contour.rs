use crate::descriptor::{Descriptor, Value};
use crate::style::fields::{flag, number};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourPoint {
    pub input: f64,
    pub output: f64,
    pub corner: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    pub name: String,
    pub points: Vec<ContourPoint>,
}

impl Default for Contour {
    fn default() -> Self {
        Self::linear()
    }
}

impl Contour {
    pub fn linear() -> Self {
        Self {
            name: "Linear".to_owned(),
            points: vec![
                ContourPoint {
                    input: 0.0,
                    output: 0.0,
                    corner: false,
                },
                ContourPoint {
                    input: 255.0,
                    output: 255.0,
                    corner: false,
                },
            ],
        }
    }

    pub fn is_linear(&self) -> bool {
        self.points.len() == 2
            && self.points[0].input == 0.0
            && self.points[0].output == 0.0
            && self.points[1].input == 255.0
            && self.points[1].output == 255.0
    }

    pub fn from_descriptor(descriptor: &Descriptor) -> Self {
        let points = descriptor
            .list("Crv ")
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_descriptor)
                    .map(|point| ContourPoint {
                        input: number(point, "Hrzn", 0.0),
                        output: number(point, "Vrtc", 0.0),
                        corner: !flag(point, "Cnty", true),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut contour = Self {
            name: descriptor.string("Nm  ").unwrap_or_default().to_owned(),
            points,
        };
        if contour.points.len() < 2 {
            contour.points = Contour::linear().points;
        }
        contour
    }

    pub fn to_descriptor(&self) -> Descriptor {
        let points = self
            .points
            .iter()
            .map(|point| {
                let mut descriptor = Descriptor::new("CrPt")
                    .with("Hrzn", point.input)
                    .with("Vrtc", point.output);
                if point.corner {
                    descriptor.set("Cnty", false);
                }
                Value::Descriptor(descriptor)
            })
            .collect::<Vec<_>>();
        Descriptor::new("ShpC")
            .with("Nm  ", self.name.as_str())
            .with("Crv ", Value::List(points))
    }

    pub fn table(&self) -> [f32; 256] {
        let curve = Spline::from_points(
            self.points
                .iter()
                .map(|point| (point.input / 255.0, point.output / 255.0)),
        );
        let mut table = [0.0f32; 256];
        for (index, slot) in table.iter_mut().enumerate() {
            *slot = curve.evaluate(index as f64 / 255.0).clamp(0.0, 1.0) as f32;
        }
        table
    }
}

pub struct Spline {
    xs: Vec<f64>,
    ys: Vec<f64>,
    second: Vec<f64>,
}

impl std::fmt::Debug for Spline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spline")
            .field("points", &self.xs.len())
            .finish()
    }
}

impl Spline {
    pub fn from_points(points: impl IntoIterator<Item = (f64, f64)>) -> Self {
        let mut points: Vec<(f64, f64)> = points.into_iter().collect();
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (xs, ys): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
        Self::new(&xs, &ys)
    }

    pub fn new(xs: &[f64], ys: &[f64]) -> Self {
        let count = xs.len();
        let mut second = vec![0.0; count];
        if count > 2 {
            let mut u = vec![0.0; count];
            for i in 1..count - 1 {
                let span = xs[i + 1] - xs[i - 1];
                if span <= 0.0 {
                    continue;
                }
                let sig = (xs[i] - xs[i - 1]) / span;
                let p = sig * second[i - 1] + 2.0;
                let right = (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i]).max(1e-12);
                let left = (ys[i] - ys[i - 1]) / (xs[i] - xs[i - 1]).max(1e-12);
                second[i] = (sig - 1.0) / p;
                u[i] = (6.0 * (right - left) / span - sig * u[i - 1]) / p;
            }
            for i in (0..count - 1).rev() {
                second[i] = second[i] * second[i + 1] + u[i];
            }
        }
        Self {
            xs: xs.to_vec(),
            ys: ys.to_vec(),
            second,
        }
    }

    pub fn evaluate(&self, x: f64) -> f64 {
        let count = self.xs.len();
        if count == 0 {
            return x;
        }
        if count == 1 || x <= self.xs[0] {
            return self.ys[0];
        }
        if x >= self.xs[count - 1] {
            return self.ys[count - 1];
        }
        let upper = self
            .xs
            .partition_point(|&value| value < x)
            .clamp(1, count - 1);
        let lower = upper - 1;
        let width = self.xs[upper] - self.xs[lower];
        if width <= 0.0 {
            return self.ys[upper];
        }
        let a = (self.xs[upper] - x) / width;
        let b = (x - self.xs[lower]) / width;
        a * self.ys[lower]
            + b * self.ys[upper]
            + ((a * a * a - a) * self.second[lower] + (b * b * b - b) * self.second[upper])
                * width
                * width
                / 6.0
    }
}
