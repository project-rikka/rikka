pub use rikka_image_resample::Kernel;

use crate::error::Result;
use crate::pixels::Plane;
use crate::transform::resample::{Edge, Resampler, Resampling};

const NEGLIGIBLE: f32 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CpuResampler {
    pub kernel: Kernel,
}

impl CpuResampler {
    pub fn new(kernel: Kernel) -> Self {
        Self { kernel }
    }
}

struct Source<'a> {
    values: &'a [f32],
    weights: Option<&'a [f32]>,
    width: i64,
    height: i64,
    edge: Edge,
}

impl Source<'_> {
    fn sample(&self, x: i64, y: i64) -> (f32, f32) {
        let inside = (0..self.width).contains(&x) && (0..self.height).contains(&y);
        let (x, y) = match (inside, self.edge) {
            (true, _) | (false, Edge::Clamp) => {
                (x.clamp(0, self.width - 1), y.clamp(0, self.height - 1))
            }
            (false, Edge::Constant(value)) => return (value, 0.0),
        };
        let index = (y * self.width + x) as usize;
        let weight = self.weights.map_or(1.0, |weights| weights[index]);
        (self.values[index], weight)
    }
}

impl Resampler for CpuResampler {
    fn resample(&mut self, request: &Resampling<'_>) -> Result<Plane> {
        let values = request.source.to_normalized();
        let weights = request.weight.map(Plane::to_normalized);
        let source = Source {
            values: &values,
            weights: weights.as_deref(),
            width: i64::from(request.source.width()),
            height: i64::from(request.source.height()),
            edge: request.edge,
        };
        let kernel = if request.discrete {
            Kernel::Nearest
        } else {
            self.kernel
        };
        let stretch_x = request.map.xx.hypot(request.map.yx).max(1.0);
        let stretch_y = request.map.xy.hypot(request.map.yy).max(1.0);
        let (reach_x, reach_y) = (kernel.support() * stretch_x, kernel.support() * stretch_y);
        let mut out = Vec::with_capacity(request.width as usize * request.height as usize);
        for row in 0..request.height {
            for column in 0..request.width {
                let (x, y) = request
                    .map
                    .apply(f64::from(column) + 0.5, f64::from(row) + 0.5);
                if kernel == Kernel::Nearest {
                    out.push(source.sample(x.floor() as i64, y.floor() as i64).0);
                    continue;
                }
                let (cx, cy) = (x - 0.5, y - 0.5);
                let mut weighted = 0.0f64;
                let mut weight_sum = 0.0f64;
                let mut plain = 0.0f64;
                let mut total = 0.0f64;
                for sy in (cy - reach_y).ceil() as i64..=(cy + reach_y).floor() as i64 {
                    let wy = kernel.weight((sy as f64 - cy) / stretch_y);
                    if wy == 0.0 {
                        continue;
                    }
                    for sx in (cx - reach_x).ceil() as i64..=(cx + reach_x).floor() as i64 {
                        let w = wy * kernel.weight((sx as f64 - cx) / stretch_x);
                        if w == 0.0 {
                            continue;
                        }
                        let (value, alpha) = source.sample(sx, sy);
                        weighted += w * f64::from(alpha) * f64::from(value);
                        weight_sum += w * f64::from(alpha);
                        plain += w * f64::from(value);
                        total += w;
                    }
                }
                let value = if request.weight.is_some() && weight_sum.abs() > f64::from(NEGLIGIBLE)
                {
                    weighted / weight_sum
                } else if total.abs() > f64::from(NEGLIGIBLE) {
                    plain / total
                } else {
                    source.sample(cx.round() as i64, cy.round() as i64).0.into()
                };
                out.push(value as f32);
            }
        }
        Plane::from_normalized(request.width, request.height, request.source.depth(), &out)
    }
}
