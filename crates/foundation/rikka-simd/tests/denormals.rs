use std::hint::black_box;

use rikka_simd::FlushDenormals;

fn halved_smallest_normal() -> f32 {
    black_box(f32::MIN_POSITIVE) * black_box(0.5)
}

#[test]
fn denormals_flush_while_guard_lives() {
    assert!(halved_smallest_normal() > 0.0);
    {
        let _flush = FlushDenormals::new();
        assert_eq!(halved_smallest_normal(), 0.0);
        assert_eq!(black_box(f32::from_bits(1)) * black_box(1.0), 0.0);
    }
    assert!(halved_smallest_normal() > 0.0);
}
