use glam::{Vec2, Vec3};
use rikka_spatial::{orient2d, orient3d};

use crate::support::{Rng, sign};

const SCALE: f32 = 1048576.0;

fn grid(rng: &mut Rng) -> f32 {
    (rng.range(-8.0, 8.0) * SCALE).round() / SCALE
}

fn scaled(value: f32) -> i128 {
    (value * SCALE) as i128
}

fn exact2(a: Vec2, b: Vec2, c: Vec2) -> i128 {
    let (ax, ay) = (scaled(a.x) - scaled(c.x), scaled(a.y) - scaled(c.y));
    let (bx, by) = (scaled(b.x) - scaled(c.x), scaled(b.y) - scaled(c.y));
    ax * by - ay * bx
}

fn exact3(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> i128 {
    let row = |p: Vec3| {
        [
            scaled(p.x) - scaled(d.x),
            scaled(p.y) - scaled(d.y),
            scaled(p.z) - scaled(d.z),
        ]
    };
    let (a, b, c) = (row(a), row(b), row(c));
    a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
        + a[2] * (b[0] * c[1] - b[1] * c[0])
}

#[test]
fn orient2d_sign_matches_exact_arithmetic_near_collinear() {
    let mut rng = Rng::new(1);
    for _ in 0..20_000 {
        let a = Vec2::new(grid(&mut rng), grid(&mut rng));
        let b = Vec2::new(grid(&mut rng), grid(&mut rng));
        let t = rng.unit();
        let c = a + (b - a) * t;
        let c = Vec2::new((c.x * SCALE).round() / SCALE, (c.y * SCALE).round() / SCALE);
        assert_eq!(sign(orient2d(a, b, c)), exact2(a, b, c).signum() as i32);
    }
}

#[test]
fn orient3d_sign_matches_exact_arithmetic_near_coplanar() {
    let mut rng = Rng::new(2);
    let point = |rng: &mut Rng| Vec3::new(grid(rng), grid(rng), grid(rng));
    for _ in 0..20_000 {
        let (a, b, c) = (point(&mut rng), point(&mut rng), point(&mut rng));
        let (s, t) = (rng.unit(), rng.unit());
        let d = a + (b - a) * s + (c - a) * t;
        let d = Vec3::new(
            (d.x * SCALE).round() / SCALE,
            (d.y * SCALE).round() / SCALE,
            (d.z * SCALE).round() / SCALE,
        );
        assert_eq!(
            sign(orient3d(a, b, c, d)),
            exact3(a, b, c, d).signum() as i32
        );
    }
}

#[test]
fn orient_conventions_follow_shewchuk() {
    assert!(orient2d(Vec2::ZERO, Vec2::X, Vec2::Y) > 0.0);
    assert!(orient3d(Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z) < 0.0);
    assert_eq!(
        orient3d(Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(5.0, -3.0, 0.0)),
        0.0
    );
    let tiny = Vec2::new(1e-30, 1e-30);
    assert_eq!(orient2d(tiny, tiny * 2.0, tiny * 3.0), 0.0);
    assert!(
        orient2d(
            Vec2::new(0.5, 0.5),
            Vec2::new(12.0, 12.0),
            Vec2::new(24.0, 24.000002)
        ) > 0.0
    );
}
