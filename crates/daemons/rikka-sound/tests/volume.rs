use rikka_sound::Volume;

fn close(one: f32, other: f32) -> bool {
    (one - other).abs() < 1e-5
}

#[test]
fn slider_is_cubic_and_keeps_channel_ratios() {
    let halfway = Volume::new(vec![1.0, 1.0], false).at_level(0.5);
    assert!(close(halfway.amplitude(), 0.125));
    assert!(close(halfway.level(), 0.5));

    let moved = Volume::new(vec![0.5, 0.25], false).at_level(1.0);
    assert!(close(moved.amplitude(), 1.0));
    assert!(close(moved.channels()[1], 0.5));

    let silent = Volume::new(vec![0.0, 0.0], false).at_level(0.5);
    assert_eq!(silent.channels(), [0.125, 0.125]);

    assert_eq!(Volume::new(vec![0.0], false).decibels(), None);
    assert!(close(
        Volume::new(vec![0.5], false).decibels().unwrap(),
        -6.0206
    ));
}

#[test]
fn balance_preserves_stereo_loudness() {
    assert!(close(Volume::new(vec![1.0, 1.0], false).balance(), 0.0));
    assert!(close(Volume::new(vec![1.0, 0.0], false).balance(), -1.0));
    assert!(close(Volume::new(vec![0.5, 1.0], false).balance(), 0.5));

    let shifted = Volume::new(vec![0.8, 0.8], false).at_balance(-0.5);
    assert!(close(shifted.amplitude(), 0.8));
    assert!(close(shifted.balance(), -0.5));

    let mono = Volume::new(vec![0.4], false);
    assert_eq!(mono.balance(), 0.0);
    assert_eq!(mono.at_balance(1.0), mono);
}
