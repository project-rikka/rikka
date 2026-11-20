use rikka_types::{Lens, RefractionExtent, Size};

#[test]
fn extent_covers_furthest_ray() {
    let roomy = Size::new(800.0, 600.0);
    let flat = Lens::new().with_bulge(0.0).with_depth(64.0);
    assert_eq!(
        flat.with_dispersion(0.0)
            .extent_for_size(roomy)
            .sample_margin,
        64.0
    );
    assert_eq!(
        flat.with_dispersion(0.5)
            .extent_for_size(roomy)
            .sample_margin,
        96.0
    );
    assert_eq!(
        flat.with_bevel(0.0).extent_for_size(roomy),
        RefractionExtent::NONE
    );
    assert_eq!(
        Lens::new().with_ior(1.0).extent_for_size(roomy),
        RefractionExtent::NONE
    );

    let bead = Lens::new()
        .with_bevel(0.0)
        .with_depth(0.0)
        .with_bulge(0.5)
        .with_dispersion(0.0);
    assert_eq!(bead.extent_for_size(roomy).sample_margin, 0.0);
    assert_eq!(bead.extent_for_size(roomy).paint_margin, 200.0);

    let fitted = Lens::new()
        .with_bevel(40.0)
        .with_depth(120.0)
        .with_bulge(0.0)
        .with_dispersion(0.0);
    assert_eq!(
        fitted.extent_for_size(Size::new(800.0, 40.0)).sample_margin,
        60.0
    );
}

#[test]
fn clamps_out_of_range_and_nan() {
    let odd = Lens::new()
        .with_bulge(f32::NAN)
        .with_frost(f32::NAN)
        .with_dispersion(f32::NAN)
        .with_adapt(f32::NAN)
        .with_bevel(f32::NAN)
        .with_depth(f32::NAN)
        .with_ior(f32::NAN);
    assert_eq!(odd, odd);
    assert_eq!(Lens::new().with_ior(0.1).refraction_strength(), 0.0);
    assert_eq!(Lens::new().with_bevel(-4.0).bevel(), 0.0);
    assert_eq!(Lens::new().with_dispersion(9.0).dispersion(), 1.0);
    assert_eq!(Lens::new().with_adapt(9.0).adapt(), 1.0);
}
