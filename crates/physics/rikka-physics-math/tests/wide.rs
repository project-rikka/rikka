use rikka_physics_math::{
    F32x4, Kernel, Level, Quat, QuatW, Simd, Vec3, Vec3W, Vector, Wide, nlerp, quat_between,
};

struct SumLengths<'a>(&'a [Vec3]);

impl Kernel for SumLengths<'_> {
    type Output = f32;

    #[inline(always)]
    fn run<W: Wide>(self) -> f32 {
        let mut total = 0.0;
        for chunk in self.0.chunks(W::LANES) {
            let v = Vec3W::<W>::from_fn(|lane| chunk.get(lane).copied().unwrap_or_default());
            total += v.length().reduce_add();
        }
        total
    }
}

#[test]
fn simd_levels_match_scalar() {
    let points: Vec<_> = (0..37)
        .map(|i| Vec3::new(i as f32, 1.0, -0.5 * i as f32))
        .collect();
    let expected: f32 = points.iter().map(|p| p.length()).sum();
    for simd in Level::supported().map(Simd::new) {
        let got = simd.run(SumLengths(&points));
        assert!(
            (got - expected).abs() < 1e-2 * expected.abs().max(1.0),
            "{}",
            simd.name()
        );
    }
}

#[test]
fn wide_quaternions_match_scalar_rotation() {
    let q = Quat::from_euler(glam::EulerRot::YXZ, 0.3, -1.2, 0.8);
    let p = Quat::from_rotation_z(0.4);
    let v = Vec3::new(0.2, -1.0, 3.0);
    let wide = QuatW::<F32x4>::splat(q);
    assert!((wide.rotate(Vec3W::splat(v)).lane(2) - q * v).length() < 1e-5);
    assert!((wide.rotate_inverse(Vec3W::splat(q * v)).lane(1) - v).length() < 1e-5);
    assert!((wide * QuatW::splat(p)).lane(0).dot(q * p).abs() > 1.0 - 1e-6);

    let from = Vec3::new(0.3, 0.8, -0.2).normalize();
    for to in [Vec3::new(-0.5, 0.1, 0.9).normalize(), -from, from] {
        let between =
            QuatW::<F32x4>::from_rotation_arc(Vec3W::splat(from), Vec3W::splat(to)).lane(3);
        assert!(between.dot(quat_between(from, to)).abs() > 1.0 - 1e-6);
        assert!((between * from - to).length() < 1e-5);
    }
    let a = Quat::from_rotation_y(0.4);
    let b = -Quat::from_rotation_x(1.1);
    let blended = QuatW::<F32x4>::splat(a)
        .nlerp(QuatW::splat(b), F32x4::splat(0.3))
        .lane(0);
    assert!(blended.dot(nlerp(a, b, 0.3)) > 1.0 - 1e-6);
}
