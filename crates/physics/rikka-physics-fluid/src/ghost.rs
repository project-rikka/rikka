use crate::lattice::packed;

const WALL_SAMPLES: usize = 64;
const SURFACE_SAMPLES: usize = 32;

pub(crate) struct Ghost {
    h: f32,
    number: f32,
    scale: f32,
}

impl Ghost {
    pub(crate) fn fitted(spacing: f32, h: f32, density: f32, offset: f32) -> Ghost {
        let unscaled = Ghost {
            h,
            number: std::f32::consts::SQRT_2 / (spacing * spacing * spacing),
            scale: 1.0,
        };
        let half: f32 = packed(spacing, h)
            .iter()
            .filter(|point| point.z > -1e-6 * spacing)
            .map(|point| (1.0 - point.length() / h).powi(2))
            .sum();
        let missing = (density - half).max(0.0);
        let (at_rest, _) = unscaled.at(offset);
        Ghost {
            scale: if at_rest > 0.0 {
                missing / at_rest
            } else {
                0.0
            },
            ..unscaled
        }
    }

    pub(crate) fn wall_table(&self) -> [[f32; 4]; WALL_SAMPLES / 2] {
        let mut table = [[0.0; 4]; WALL_SAMPLES / 2];
        for sample in 0..WALL_SAMPLES {
            let (value, slope) = self.at(self.h * sample as f32 / (WALL_SAMPLES - 1) as f32);
            let entry = &mut table[sample / 2];
            let lane = 2 * (sample % 2);
            entry[lane] = value;
            entry[lane + 1] = slope;
        }
        table
    }

    pub(crate) fn surface_table(&self, density: f32) -> ([[f32; 4]; SURFACE_SAMPLES / 4], f32) {
        let steepness = |depth: f32| -self.at(depth).1 * self.h / density;
        let steepest = steepness(0.0);
        let mut table = [[1.0; 4]; SURFACE_SAMPLES / 4];
        for sample in 0..SURFACE_SAMPLES {
            let threshold = steepest * sample as f32 / (SURFACE_SAMPLES - 1) as f32;
            let (mut near, mut far) = (0.0, self.h);
            for _ in 0..24 {
                let middle = 0.5 * (near + far);
                if steepness(middle) > threshold {
                    near = middle
                } else {
                    far = middle
                }
            }
            let (missing, _) = self.at(0.5 * (near + far));
            table[sample / 4][sample % 4] = 1.0 - missing / density;
        }
        (table, steepest)
    }

    fn at(&self, depth: f32) -> (f32, f32) {
        let steps = 256;
        let width = (self.h - depth).max(0.0) / steps as f32;
        let mut value = 0.0;
        let mut slope = 0.0;
        for step in 0..steps {
            let r = depth + (step as f32 + 0.5) * width;
            let w = (1.0 - r / self.h).powi(2);
            value += w * std::f32::consts::TAU * r * (r - depth) * width;
            slope -= w * std::f32::consts::TAU * r * width;
        }
        let scaled = self.number * self.scale;
        (scaled * value, scaled * slope)
    }
}
