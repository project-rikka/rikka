#[derive(Clone, Debug)]
pub struct Spline {
    xs: Vec<f64>,
    ys: Vec<f64>,
    second: Vec<f64>,
}

impl Spline {
    #[must_use]
    pub fn from_points(points: impl IntoIterator<Item = (f64, f64)>) -> Spline {
        let mut points: Vec<(f64, f64)> = points.into_iter().collect();
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (xs, ys): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
        Spline::new(&xs, &ys)
    }

    pub(crate) fn new(xs: &[f64], ys: &[f64]) -> Spline {
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
        Spline {
            xs: xs.to_vec(),
            ys: ys.to_vec(),
            second,
        }
    }

    #[must_use]
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
