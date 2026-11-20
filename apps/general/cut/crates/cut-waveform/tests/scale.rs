use cut_waveform::Scale;

#[test]
fn scale_maps_amplitude_with_gain() {
    let decibels = |db: f32| 10f32.powf(db / 20.0);
    for (scale, amplitude, gain, reach) in [
        (Scale::Linear, 0.5, 1.0, 0.5),
        (Scale::Linear, -0.5, 2.0, 1.0),
        (Scale::Linear, 0.8, 2.0, 1.0),
        (Scale::Linear, 0.5, 0.0, 0.0),
        (Scale::Logarithmic, 1.0, 1.0, 1.0),
        (Scale::Logarithmic, decibels(-24.0), 1.0, 0.5),
        (Scale::Logarithmic, decibels(-30.0), decibels(6.0), 0.5),
        (Scale::Logarithmic, decibels(-60.0), 1.0, 0.0),
        (Scale::Logarithmic, 0.0, 1.0, 0.0),
        (Scale::Logarithmic, 0.5, 0.0, 0.0),
    ] {
        let actual = scale.level(amplitude, gain);
        assert!(
            (actual - reach).abs() < 1e-4,
            "{scale:?} {amplitude} × {gain} reached {actual}, not {reach}"
        );
    }
    assert!(Scale::clips(0.8, 1.5) && !Scale::clips(0.8, 1.0));
}
