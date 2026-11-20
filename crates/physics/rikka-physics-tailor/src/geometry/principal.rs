use glam::{DVec3, Vec3};

const POWER_STEPS: usize = 64;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Principal {
    pub(crate) centre: Vec3,
    pub(crate) axes: [Vec3; 3],
    pub(crate) half: [f32; 3],
}

impl Principal {
    pub(crate) fn compute(points: &[Vec3]) -> Option<Principal> {
        if points.is_empty() {
            return None;
        }
        let sum: DVec3 = points.iter().map(|p| p.as_dvec3()).sum();
        let centroid = sum / points.len() as f64;
        let mut spread = [DVec3::ZERO; 3];
        for point in points {
            let d = point.as_dvec3() - centroid;
            spread[0] += d * d.x;
            spread[1] += d * d.y;
            spread[2] += d * d.z;
        }
        let first = dominant(spread, DVec3::X);
        let weight = apply(spread, first).dot(first);
        let deflated = [
            spread[0] - first * (weight * first.x),
            spread[1] - first * (weight * first.y),
            spread[2] - first * (weight * first.z),
        ];
        let second = dominant(deflated, first.any_orthogonal_vector());
        let second = (second - first * second.dot(first))
            .try_normalize()
            .unwrap_or_else(|| first.any_orthogonal_vector().normalize());
        let axes = [first, second, first.cross(second)].map(|axis| axis.as_vec3());
        let centroid = centroid.as_vec3();
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        for point in points {
            for (index, axis) in axes.iter().enumerate() {
                let along = (*point - centroid).dot(*axis);
                low[index] = low[index].min(along);
                high[index] = high[index].max(along);
            }
        }
        let centre = (0..3).fold(centroid, |centre, index| {
            centre + axes[index] * ((low[index] + high[index]) * 0.5)
        });
        Some(Principal {
            centre,
            axes,
            half: std::array::from_fn(|index| (high[index] - low[index]) * 0.5),
        })
    }

    pub(crate) fn local(&self, point: Vec3) -> Vec3 {
        let offset = point - self.centre;
        Vec3::new(
            offset.dot(self.axes[0]),
            offset.dot(self.axes[1]),
            offset.dot(self.axes[2]),
        )
    }
}

fn apply(columns: [DVec3; 3], v: DVec3) -> DVec3 {
    columns[0] * v.x + columns[1] * v.y + columns[2] * v.z
}

fn dominant(columns: [DVec3; 3], fallback: DVec3) -> DVec3 {
    let Some(mut axis) = columns
        .iter()
        .copied()
        .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        .and_then(DVec3::try_normalize)
    else {
        return fallback.normalize();
    };
    for _ in 0..POWER_STEPS {
        match apply(columns, axis).try_normalize() {
            Some(next) => axis = next,
            None => break,
        }
    }
    axis
}
