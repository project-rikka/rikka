use rikka_network::Bars;

#[test]
fn signal_bars_step_each_quarter() {
    for (strength, bars) in [
        (0, Bars::One),
        (24, Bars::One),
        (25, Bars::Two),
        (49, Bars::Two),
        (50, Bars::Three),
        (74, Bars::Three),
        (75, Bars::Four),
        (100, Bars::Four),
    ] {
        assert_eq!(Bars::from_strength(strength), bars, "{strength}");
    }
}
