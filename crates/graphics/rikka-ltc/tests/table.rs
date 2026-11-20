use std::f64::consts::PI;

use glam::{DMat3, DVec3};
use rikka_ltc::{ENTRY_BYTES, GGX, SIZE, Table, fit_roughness, sphere};

fn committed() -> Table {
    Table::from_bytes(GGX).expect("committed table has SIZE² entries")
}

#[test]
fn committed_table_matches_a_refit_of_its_roughest_rows() {
    assert_eq!(GGX.len(), SIZE * SIZE * ENTRY_BYTES);
    let table = committed();
    let lowest = SIZE - 6;
    let rows = fit_roughness(lowest);
    for (offset, row) in rows.iter().enumerate() {
        for (y, refit) in row.iter().enumerate() {
            let stored = table.entry(lowest + offset, y);
            let close = |a: f32, b: f32| (a - b).abs() <= 1e-4 * a.abs().max(b.abs()).max(1.0);
            assert!(
                refit
                    .inverse
                    .iter()
                    .zip(stored.inverse)
                    .all(|(&a, b)| close(a, b))
                    && close(refit.magnitude, stored.magnitude)
                    && close(refit.fresnel, stored.fresnel),
                "entry ({}, {y}) differs: {refit:?} vs {stored:?}",
                lowest + offset
            );
        }
    }
}

#[test]
fn sphere_clipping_spans_full_to_hidden() {
    let cases = [
        (1.0, 0.5, 1.0),
        (0.5, 0.25, 0.5),
        (0.5, 0.0, 0.5),
        (-0.5, 0.25, 0.0),
        (-1.0, 0.5, 0.0),
    ];
    for (cosine, form_factor, expected) in cases {
        let clipped = sphere(cosine, form_factor) as f64;
        assert!(
            (clipped - expected).abs() < 0.02,
            "sphere({cosine}, {form_factor}) = {clipped}, expected {expected}"
        );
    }
}

fn lambda(alpha: f64, cosine: f64) -> f64 {
    let tangent = (1.0 - cosine * cosine).max(0.0).sqrt() / cosine;
    let a = 1.0 / (alpha * tangent);
    0.5 * (-1.0 + (1.0 + 1.0 / (a * a)).sqrt())
}

fn ggx_cosine(view: DVec3, light: DVec3, alpha: f64) -> f64 {
    if light.z <= 0.0 {
        return 0.0;
    }
    let half = (view + light).normalize();
    let a2 = alpha * alpha;
    let d = half.z * half.z * (a2 - 1.0) + 1.0;
    let distribution = a2 / (PI * d * d);
    let shadowing = 1.0 / (1.0 + lambda(alpha, view.z) + lambda(alpha, light.z));
    distribution * shadowing / (4.0 * view.z)
}

fn polygon_form_factor(corners: [DVec3; 4]) -> f64 {
    let unit = corners.map(DVec3::normalize);
    let mut sum = 0.0;
    for index in 0..4 {
        let (a, b) = (unit[index], unit[(index + 1) % 4]);
        let angle = a.dot(b).clamp(-1.0, 1.0).acos();
        sum += angle * a.cross(b).normalize().z;
    }
    (sum / (2.0 * PI)).abs()
}

#[test]
fn rect_light_integral_matches_brute_force_ggx() {
    let table = committed();
    let cases = [(19, 20), (32, 40), (51, 52)];
    for (x, y) in cases {
        let roughness = x as f64 / (SIZE - 1) as f64;
        let alpha = roughness * roughness;
        let along = y as f64 / (SIZE - 1) as f64;
        let cosine = 1.0 - along * along;
        let view = DVec3::new((1.0 - cosine * cosine).sqrt(), 0.0, cosine);
        let reflected = DVec3::new(-view.x, 0.0, view.z);
        let center = reflected * 2.0;
        let across = DVec3::Y * 0.6;
        let up = reflected.cross(DVec3::Y).normalize() * 0.6;
        let corners = [
            center - across - up,
            center + across - up,
            center + across + up,
            center - across + up,
        ];

        let mut expected = 0.0;
        let steps = 400;
        let normal = reflected;
        let patch = (1.2 / steps as f64) * (1.2 / steps as f64);
        for i in 0..steps {
            for j in 0..steps {
                let s = (i as f64 + 0.5) / steps as f64 * 2.0 - 1.0;
                let t = (j as f64 + 0.5) / steps as f64 * 2.0 - 1.0;
                let point = center + across * s + up * t;
                let distance = point.length();
                let light = point / distance;
                let solid = patch * light.dot(normal).abs() / (distance * distance);
                expected += ggx_cosine(view, light, alpha) * solid;
            }
        }

        let entry = table.entry(x, y);
        let [a, b, c, d] = entry.inverse.map(f64::from);
        let inverse = DMat3::from_cols(DVec3::new(a, 0.0, b), DVec3::Y, DVec3::new(c, 0.0, d));
        let fitted = polygon_form_factor(corners.map(|corner| inverse * corner))
            * f64::from(entry.magnitude);
        let error = (fitted - expected).abs() / expected;
        assert!(
            error < 0.12,
            "roughness {roughness}, n·v {cosine}: LTC {fitted} vs GGX {expected}"
        );
    }
}
