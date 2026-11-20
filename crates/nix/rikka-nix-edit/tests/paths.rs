use rikka_nix_edit::{AttrPath, IntoAttrPath};

#[test]
fn attr_path_round_trips() {
    let written = r#"systemd.user.units."app-org.fcitx.Fcitx5@autostart.service".enable"#;
    let path = AttrPath::parse(written).expect("it is a path");
    assert_eq!(
        path.segments(),
        [
            "systemd",
            "user",
            "units",
            "app-org.fcitx.Fcitx5@autostart.service",
            "enable"
        ]
    );
    assert_eq!(path.to_string(), written);

    let handed = ["users", "users", "a \"b\" ${c}", "extraGroups"]
        .into_attr_path()
        .expect("any segment is a name");
    assert_eq!(AttrPath::parse(&handed.to_string()), Ok(handed));
}

#[test]
fn rejects_dynamic_attr_path() {
    for text in [
        "",
        "a.",
        ".a",
        "a = 1; b",
        "a.${b}",
        r#"a."${b}""#,
        "a b",
        "a; } // { b",
    ] {
        assert!(AttrPath::parse(text).is_err(), "{text}");
    }
    assert!(Vec::<String>::new().into_attr_path().is_err());
}
