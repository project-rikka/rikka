use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Lattice {
    pub(crate) density: f32,
    pub(crate) stiffness: f32,
    pub(crate) neighbours: f32,
    pub(crate) spread: f32,
    pub(crate) fullness: f32,
}

impl Lattice {
    pub(crate) fn measure(spacing: f32, h: f32) -> Lattice {
        let points = packed(spacing, h);
        let mut density = 0.0;
        let mut stiffness = 0.0;
        let mut weights = 0.0;
        let mut second = 0.0;
        for point in &points {
            let r = point.length();
            let q = 1.0 - r / h;
            density += q * q;
            let slope = 2.0 * q / h;
            stiffness += slope * slope;
            let w = 1.0 - (r / h).powi(3);
            weights += w;
            second += w * point.x * point.x;
        }
        let half = 1.0 - 0.125;
        Lattice {
            density,
            stiffness,
            neighbours: points.len() as f32,
            spread: (second / weights).sqrt(),
            fullness: 4.0 * half,
        }
    }
}

pub(crate) fn packed(spacing: f32, reach: f32) -> Vec<Vec3> {
    let side = spacing * std::f32::consts::SQRT_2;
    let cells = (reach / side).ceil() as i32 + 1;
    let basis = [
        Vec3::ZERO,
        Vec3::new(0.5, 0.5, 0.0),
        Vec3::new(0.5, 0.0, 0.5),
        Vec3::new(0.0, 0.5, 0.5),
    ];
    let mut points = Vec::new();
    for z in -cells..=cells {
        for y in -cells..=cells {
            for x in -cells..=cells {
                for offset in basis {
                    let point = (Vec3::new(x as f32, y as f32, z as f32) + offset) * side;
                    let r = point.length();
                    if r > 0.0 && r < reach {
                        points.push(point);
                    }
                }
            }
        }
    }
    points.sort_by(|a, b| {
        a.length_squared()
            .total_cmp(&b.length_squared())
            .then(a.x.total_cmp(&b.x))
            .then(a.y.total_cmp(&b.y))
            .then(a.z.total_cmp(&b.z))
    });
    points
}

pub(crate) fn filled_sphere(spacing: f32, radius: f32) -> Vec<Vec3> {
    let mut points = vec![Vec3::ZERO];
    points.extend(packed(spacing, radius.max(0.0)));
    points
}
