use installer_core::stable_release;

#[test]
fn installs_newest_stable_release() {
    let cases = [
        ("26.05.20260830.9a1b2c3 (Yarara)\n", Some("26.05")),
        ("25.11.1234.abcdef0 (Xantusia)", Some("25.11")),
        ("26.05beta1234.abcdef0 (Yarara)", Some("26.05")),
        ("26.11pre880000.abcdef0 (Zokor)", Some("26.05")),
        ("27.05pre900000.abcdef0", Some("26.11")),
        ("26.11.20261011.7fd4004 (Zokor)", Some("26.11")),
        ("", None),
        ("unknown", None),
        ("26.07.1.abc", None),
    ];
    for (version, expected) in cases {
        assert_eq!(stable_release(version).as_deref(), expected, "{version:?}");
    }
}
