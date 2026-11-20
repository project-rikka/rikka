use nixpkgs_search::Pin;

#[test]
fn pin_parses_nixos_version() {
    let cases = [
        (
            "unstable",
            "26.11.20261011.7fd4004",
            Some(("26.11", "7fd4004")),
        ),
        (
            "unstable",
            "26.11pre1090327.7fd4004d0511\n",
            Some(("26.11", "7fd4004d0511")),
        ),
        (
            "26.05",
            "26.05.9989.ECC58F32D106",
            Some(("26.05", "ecc58f32d106")),
        ),
        (
            "26.05",
            "26.05beta1.705e9929918b",
            Some(("26.05", "705e9929918b")),
        ),
        ("unstable", "26.11.19700101.dirty", None),
        ("unstable", "26.11.20261011.7fd400", None),
        ("unstable", "", None),
    ];
    for (channel, version, expected) in cases {
        let pin = Pin::new(channel, version);
        assert_eq!(pin.channel(), channel);
        assert_eq!(
            pin.release().zip(pin.commit()),
            expected,
            "{channel} {version:?}"
        );
    }
}
