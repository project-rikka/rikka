const SIDE: u32 = 96;

type Shape = fn(f32, f32) -> u8;

fn drawn(alpha: impl Fn(f32, f32) -> u8) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (u, v) = (
                (x as f32 + 0.5) / SIDE as f32,
                (y as f32 + 0.5) / SIDE as f32,
            );
            rgba.extend([200, 100, 50, alpha(u, v)]);
        }
    }
    rgba
}

fn full_bleed_square(_: f32, _: f32) -> u8 {
    255
}

fn full_bleed_disc(u: f32, v: f32) -> u8 {
    match (u - 0.5).hypot(v - 0.5) <= 0.5 {
        true => 255,
        false => 0,
    }
}

fn padded_square(u: f32, v: f32) -> u8 {
    match (0.2..0.8).contains(&u) && (0.2..0.8).contains(&v) {
        true => 255,
        false => 0,
    }
}

fn faint_halo_around_padded_square(u: f32, v: f32) -> u8 {
    match padded_square(u, v) {
        0 => 30,
        solid => solid,
    }
}

fn scaled_share(scale: f32, covered: f32) -> f32 {
    scale * scale * covered
}

#[test]
fn shapes_reach_same_visible_area() {
    let cases: [(&str, Shape, f32, f32); 4] = [
        ("full-bleed square", full_bleed_square, 1.0, 375.0 / 576.0),
        (
            "full-bleed disc",
            full_bleed_disc,
            std::f32::consts::FRAC_PI_4,
            380.0 / 576.0,
        ),
        ("padded square", padded_square, 0.36, 0.36),
        (
            "padded square in a faint halo",
            faint_halo_around_padded_square,
            0.36,
            0.36,
        ),
    ];
    for (name, shape, covered, expected) in cases {
        let scale = icon_footprint::scale(SIDE, SIDE, &drawn(shape)).expect(name);
        assert!(scale > 0.0 && scale <= 1.0, "{name}: scale {scale}");
        let share = scaled_share(scale, covered);
        assert!(
            (share - expected).abs() < 0.02,
            "{name}: visible share {share}, expected {expected}"
        );
    }
}

#[test]
fn empty_or_mismatched_raster_has_no_scale() {
    let invisible = drawn(|_, _| 0);
    let short = &drawn(full_bleed_square)[4..];

    assert_eq!(icon_footprint::scale(SIDE, SIDE, &invisible), None);
    assert_eq!(icon_footprint::scale(SIDE, SIDE, short), None);
    assert_eq!(icon_footprint::scale(0, 0, &[]), None);
}
