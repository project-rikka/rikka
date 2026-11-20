use animate_body::Side;
use animate_hold::{Hold, HoldFile, HoldTarget, Holder, Holds};
use rikka_model_core::glam::Vec3;

#[test]
fn holds_file_round_trip() {
    let mut holds = Holds::default();
    holds.lay(Hold {
        until: Some(48),
        ease: 4,
        ..Hold::new(
            Holder::Hand(Side::Left),
            HoldTarget {
                figure: Some("Miku".into()),
                at: Vec3::new(0.12, 0.71, 0.04),
                reach: 0.06,
            },
            12,
        )
    });
    holds.lay(Hold::new(
        Holder::Pin {
            node: 42,
            at: Vec3::new(0.0, -0.1, 0.2),
        },
        HoldTarget {
            figure: None,
            at: Vec3::new(-0.1, 0.6, 0.0),
            reach: 0.04,
        },
        30,
    ));
    let text = serde_json::to_string(&HoldFile::from_holds(&holds)).expect("holds write");
    let files: Vec<HoldFile> = serde_json::from_str(&text).expect("holds read");
    assert_eq!(HoldFile::holds(&files).expect("holds rebuild"), holds);
}

#[test]
fn rejects_invalid_hold() {
    let cases = [
        r#"[{"holder":{"hand":"left"},"cloth":{"at":[0,0,0],"reach":0},"from":0,"ease":6}]"#,
        r#"[{"holder":{"hand":"right"},"cloth":{"at":[0,0,0],"reach":0.1},"from":9,"until":9,"ease":6}]"#,
        r#"[{"holder":{"pin":{"node":1,"at":[0,0,0]}},"cloth":{"at":[0,0,0],"reach":0.1},"from":0,"ease":999}]"#,
    ];
    for case in cases {
        let files: Vec<HoldFile> = serde_json::from_str(case).expect("the form parses");
        assert!(HoldFile::holds(&files).is_err(), "{case}");
    }
}
