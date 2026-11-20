use rikka_hifi_core::{Gains, Loudness, ReplayGain, ReplayGainMode, amplitude};

#[test]
fn replay_gain_mode_and_fallback() {
    let both = Gains::default()
        .track(Loudness::new(-6.0).peak(0.5))
        .album(Loudness::new(-4.0).peak(0.9));
    let track_only = Gains::default().track(Loudness::new(-6.0));
    let album_only = Gains::default().album(Loudness::new(-4.0));
    let loud = Gains::default().track(Loudness::new(5.0).peak(0.8));
    let cases = [
        (ReplayGain::new(ReplayGainMode::Off), both, 0.0),
        (ReplayGain::new(ReplayGainMode::Track), both, -6.0),
        (ReplayGain::new(ReplayGainMode::Album), both, -4.0),
        (ReplayGain::new(ReplayGainMode::Album), track_only, -6.0),
        (ReplayGain::new(ReplayGainMode::Track), album_only, -4.0),
        (
            ReplayGain::new(ReplayGainMode::Track),
            Gains::default(),
            0.0,
        ),
        (
            ReplayGain::new(ReplayGainMode::Track).preamp(3.0),
            track_only,
            -3.0,
        ),
        (
            ReplayGain::new(ReplayGainMode::Track),
            loud,
            -20.0 * 0.8_f32.log10(),
        ),
        (
            ReplayGain::new(ReplayGainMode::Track).prevent_clipping(false),
            loud,
            5.0,
        ),
    ];
    for (settings, gains, expected) in cases {
        let applied = settings.applied_db(&gains);
        assert!(
            (applied - expected).abs() < 1e-4,
            "{settings:?} on {gains:?} gave {applied}, not {expected}"
        );
    }
}

#[test]
fn zero_decibels_is_exact_unity() {
    assert_eq!(amplitude(0.0), 1.0);
    assert!((amplitude(-6.0206) - 0.5).abs() < 1e-4);
}
