use std::f64::consts::PI;

use glam::{DMat3, DVec3};

use crate::ggx;
use crate::lobe::Lobe;
use crate::simplex::{self, Search};
use crate::{Entry, SIZE};

const SAMPLES: usize = 32;

const SPHERE_SAMPLES: usize = 96;

const MIN_ALPHA: f64 = 1e-5;

const LARGEST_ANGLE: f64 = 1.57;

const SEARCH: Search = Search {
    step: 0.05,
    tolerance: 1e-5,
    iterations: 100,
};

struct Moments {
    magnitude: f64,
    fresnel: f64,
    direction: DVec3,
}

fn view_of(theta_index: usize) -> DVec3 {
    let x = theta_index as f64 / (SIZE - 1) as f64;
    let theta = (1.0 - x * x).acos().min(LARGEST_ANGLE);
    DVec3::new(theta.sin(), 0.0, theta.cos())
}

fn alpha_of(roughness_index: usize) -> f64 {
    let roughness = roughness_index as f64 / (SIZE - 1) as f64;
    (roughness * roughness).max(MIN_ALPHA)
}

fn stratum(index: usize) -> f64 {
    (index as f64 + 0.5) / SAMPLES as f64
}

fn moments(view: DVec3, alpha: f64) -> Moments {
    let mut magnitude = 0.0;
    let mut fresnel = 0.0;
    let mut direction = DVec3::ZERO;
    for j in 0..SAMPLES {
        for i in 0..SAMPLES {
            let light = ggx::sample(view, alpha, stratum(i), stratum(j));
            let evaluated = ggx::evaluate(view, light, alpha);
            if evaluated.pdf <= 0.0 {
                continue;
            }
            let weight = evaluated.value / evaluated.pdf;
            let half = (view + light).normalize();
            magnitude += weight;
            fresnel += weight * (1.0 - view.dot(half).max(0.0)).powi(5);
            direction += weight * light;
        }
    }
    let count = (SAMPLES * SAMPLES) as f64;
    direction.y = 0.0;
    Moments {
        magnitude: magnitude / count,
        fresnel: fresnel / count,
        direction: direction.normalize(),
    }
}

fn mismatch(lobe: &Lobe, view: DVec3, alpha: f64) -> f64 {
    let mut error = 0.0;
    let mut add = |light: DVec3| {
        let brdf = ggx::evaluate(view, light, alpha);
        let fitted = lobe.evaluate(light);
        let fitted_pdf = fitted / lobe.magnitude;
        if brdf.pdf > 0.0 {
            error += (brdf.value - fitted).abs().powi(3) / (fitted_pdf + brdf.pdf);
        }
    };
    for j in 0..SAMPLES {
        for i in 0..SAMPLES {
            add(lobe.sample(stratum(i), stratum(j)));
            add(ggx::sample(view, alpha, stratum(i), stratum(j)));
        }
    }
    error / (SAMPLES * SAMPLES) as f64
}

fn shaped(lobe: &mut Lobe, parameters: &[f64; 3], isotropic: bool) {
    let m11 = parameters[0].max(1e-7);
    let m22 = parameters[1].max(1e-7);
    (lobe.m11, lobe.m22, lobe.m13) = match isotropic {
        true => (m11, m11, 0.0),
        false => (m11, m22, parameters[2]),
    };
    lobe.update();
}

fn refine(lobe: &mut Lobe, view: DVec3, alpha: f64, isotropic: bool) {
    let start = [lobe.m11, lobe.m22, lobe.m13];
    let mut trial = *lobe;
    let best = simplex::minimize(start, &SEARCH, |parameters| {
        shaped(&mut trial, parameters, isotropic);
        mismatch(&trial, view, alpha)
    });
    shaped(lobe, &best, isotropic);
}

fn fitted(lobe: &mut Lobe, roughness_index: usize, theta_index: usize) -> Entry {
    let view = view_of(theta_index);
    let alpha = alpha_of(roughness_index);
    let moments = moments(view, alpha);
    lobe.magnitude = moments.magnitude;
    lobe.fresnel = moments.fresnel;
    let isotropic = theta_index == 0;
    lobe.frame = match isotropic {
        true => DMat3::IDENTITY,
        false => {
            let toward = moments.direction;
            DMat3::from_cols(DVec3::new(toward.z, 0.0, -toward.x), DVec3::Y, toward)
        }
    };
    if isotropic {
        lobe.m13 = 0.0;
    }
    lobe.update();
    refine(lobe, view, alpha, isotropic);
    let inverse = lobe.matrix().inverse();
    let inverse = inverse / inverse.y_axis.y;
    Entry {
        inverse: [
            inverse.x_axis.x as f32,
            inverse.x_axis.z as f32,
            inverse.z_axis.x as f32,
            inverse.z_axis.z as f32,
        ],
        magnitude: lobe.magnitude as f32,
        fresnel: lobe.fresnel as f32,
        sphere: 0.0,
    }
}

pub(crate) fn axis(roughness_index: usize, previous: Option<&Lobe>) -> (Lobe, Entry) {
    let mut lobe = Lobe::new();
    if let Some(previous) = previous {
        lobe.m11 = previous.m11;
        lobe.m22 = previous.m22;
    }
    let entry = fitted(&mut lobe, roughness_index, 0);
    (lobe, entry)
}

pub(crate) fn row(roughness_index: usize, axis: Lobe, first: Entry) -> Vec<Entry> {
    let mut lobe = axis;
    let mut entries = Vec::with_capacity(SIZE);
    entries.push(first);
    for theta_index in 1..SIZE {
        entries.push(fitted(&mut lobe, roughness_index, theta_index));
    }
    entries
}

pub(crate) fn sphere(cosine: f64, form_factor: f64) -> f64 {
    if form_factor <= 0.0 {
        return cosine.max(0.0);
    }
    let radius = form_factor.min(1.0).sqrt().asin();
    let elevation = cosine.clamp(-1.0, 1.0).acos();
    let (sin_e, cos_e) = elevation.sin_cos();
    let steps = SPHERE_SAMPLES;
    let d_beta = radius / steps as f64;
    let d_phi = 2.0 * PI / (2 * steps) as f64;
    let mut sum = 0.0;
    for b in 0..steps {
        let beta = (b as f64 + 0.5) * d_beta;
        let (sin_b, cos_b) = beta.sin_cos();
        for p in 0..2 * steps {
            let phi = (p as f64 + 0.5) * d_phi;
            let along = sin_e * sin_b * phi.cos() + cos_e * cos_b;
            sum += along.max(0.0) * sin_b;
        }
    }
    sum * d_beta * d_phi / PI / form_factor
}
