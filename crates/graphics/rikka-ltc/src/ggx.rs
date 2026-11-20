use std::f64::consts::PI;

use glam::DVec3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Evaluated {
    pub value: f64,
    pub pdf: f64,
}

impl Evaluated {
    const NONE: Self = Self {
        value: 0.0,
        pdf: 0.0,
    };
}

fn lambda(alpha: f64, cosine: f64) -> f64 {
    if cosine >= 1.0 {
        return 0.0;
    }
    let tangent = (1.0 - cosine * cosine).max(0.0).sqrt() / cosine;
    let a = 1.0 / (alpha * tangent);
    0.5 * (-1.0 + (1.0 + 1.0 / (a * a)).sqrt())
}

pub(crate) fn evaluate(view: DVec3, light: DVec3, alpha: f64) -> Evaluated {
    if view.z <= 0.0 {
        return Evaluated::NONE;
    }
    let masking = lambda(alpha, view.z);
    let shadowing = match light.z <= 0.0 {
        true => 0.0,
        false => 1.0 / (1.0 + masking + lambda(alpha, light.z)),
    };
    let half = (view + light).normalize();
    let slope_x = half.x / half.z;
    let slope_y = half.y / half.z;
    let d = 1.0 / (1.0 + (slope_x * slope_x + slope_y * slope_y) / (alpha * alpha));
    let distribution = d * d / (PI * alpha * alpha * half.z.powi(4));
    Evaluated {
        value: distribution * shadowing / 4.0 / view.z,
        pdf: (distribution * half.z / 4.0 / view.dot(half)).abs(),
    }
}

pub(crate) fn sample(view: DVec3, alpha: f64, u1: f64, u2: f64) -> DVec3 {
    let phi = 2.0 * PI * u1;
    let radius = alpha * (u2 / (1.0 - u2)).sqrt();
    let normal = DVec3::new(radius * phi.cos(), radius * phi.sin(), 1.0).normalize();
    -view + 2.0 * normal * normal.dot(view)
}
