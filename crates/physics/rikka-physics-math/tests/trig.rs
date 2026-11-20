use std::f32::consts::PI;

use rikka_physics_math::{atan2, cos_sin};

#[test]
fn fast_trig_matches_libm() {
    let mut angle = -20.0f32;
    while angle < 20.0 {
        let cs = cos_sin(angle);
        assert!((cs.cosine - angle.cos()).abs() < 2e-6, "cos {angle}");
        assert!((cs.sine - angle.sin()).abs() < 2e-6, "sin {angle}");
        angle += 0.0137;
    }

    let mut angle = -PI;
    while angle < PI {
        for radius in [0.01f32, 1.0, 300.0] {
            let (s, c) = angle.sin_cos();
            let expected = (radius * s).atan2(radius * c);
            let got = atan2(radius * s, radius * c);
            assert!(
                (got - expected).abs() < 2e-6,
                "atan2 {angle}: {got} vs {expected}"
            );
        }
        angle += 0.0091;
    }
    assert_eq!(atan2(0.0, 0.0), 0.0);
}
