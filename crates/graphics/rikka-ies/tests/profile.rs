use std::f32::consts::PI;

use rikka_ies::{Edition, IesError, IesProfile, Symmetry};

const BILATERAL: &str = "IESNA:LM-63-2002
[TEST] 1234
[MANUFAC] Rikka
[LUMINAIRE] Test luminaire
[MORE] second line
TILT=NONE
1 -1 2.0 5 3
1 2 0.3 0.6 0.1
1.0 1.0 40
0 22.5 45 67.5 90
0 90 180
100 90 70 40 0
100 80 50 20 0
100,60,30,10,0
";

fn profile(header: &str, lamp_factor: f32, horizontal: &[f32]) -> String {
    let vertical = [0.0, 45.0, 90.0];
    let mut text = format!(
        "{header}\n[TEST] t\nTILT=NONE\n2 1500 1 {} {} 1 2 0 0 0\n1 {lamp_factor} 30\n",
        vertical.len(),
        horizontal.len()
    );
    let list = |angles: &[f32]| {
        angles
            .iter()
            .map(f32::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    };
    text += &format!("{}\n{}\n", list(&vertical), list(horizontal));
    for (row, angle) in horizontal.iter().enumerate() {
        let values = [100.0 + angle, 50.0 + row as f32, 0.0];
        text += &format!("{}\n", list(&values));
    }
    text
}

fn parsed(text: &str) -> IesProfile {
    IesProfile::parse(text.as_bytes()).unwrap_or_else(|error| panic!("{error}: {text}"))
}

#[test]
fn reads_the_tabulated_candela_scaled_by_the_multiplier() {
    let profile = parsed(BILATERAL);
    assert_eq!(profile.edition(), Edition::Lm2002);
    assert_eq!(profile.symmetry(), Symmetry::Bilateral);
    assert_eq!(profile.vertical_angles(), [0.0, 22.5, 45.0, 67.5, 90.0]);
    assert_eq!(profile.horizontal_angles(), [0.0, 90.0, 180.0]);
    assert_eq!(profile.tabulated(1, 2), Some(100.0));
    assert_eq!(profile.tabulated(2, 3), Some(20.0));
    assert_eq!(profile.tabulated(3, 0), None);
    assert_eq!(profile.peak_candela(), 200.0);
    assert_eq!(profile.lamp_lumens(), None);
    assert_eq!(profile.input_watts(), 40.0);
    assert_eq!(
        profile.keyword("luminaire"),
        Some("Test luminaire\nsecond line")
    );
    let cases = [
        (45.0, 90.0, 100.0),
        (45.0, 270.0, 100.0),
        (45.0, 45.0, 120.0),
        (33.75, 0.0, 160.0),
        (45.0, 135.0, 80.0),
        (90.0, 30.0, 0.0),
        (120.0, 0.0, 0.0),
    ];
    for (vertical, horizontal, expected) in cases {
        let candela = profile.candela(vertical, horizontal);
        assert!(
            (candela - expected).abs() < 1e-3,
            "({vertical}°, {horizontal}°) gives {candela}, not {expected}"
        );
    }
}

#[test]
fn reads_every_edition_header() {
    let cases = [
        ("Free text of a 1986 file", Edition::Lm1986, 50.0),
        ("IESNA91", Edition::Lm1991, 50.0),
        ("IESNA:LM-63-1995", Edition::Lm1995, 50.0),
        ("IESNA:LM-63-2002", Edition::Lm2002, 100.0),
        ("IES:LM-63-2019", Edition::Lm2019, 100.0),
    ];
    for (header, edition, peak) in cases {
        let profile = parsed(&profile(header, 0.5, &[0.0]));
        assert_eq!(profile.edition(), edition, "{header}");
        assert_eq!(profile.peak_candela(), peak, "{header}");
        assert_eq!(profile.lamp_lumens(), Some(3000.0), "{header}");
        assert_eq!(profile.keyword("TEST"), Some("t"), "{header}");
    }
}

struct Mirrored {
    horizontal: &'static [f32],
    symmetry: Symmetry,
    pairs: &'static [(f32, f32)],
}

#[test]
fn symmetric_profiles_mirror_into_the_full_circle() {
    let cases = [
        Mirrored {
            horizontal: &[0.0, 45.0, 90.0],
            symmetry: Symmetry::Quadrant,
            pairs: &[(30.0, 150.0), (30.0, 210.0), (30.0, 330.0), (90.0, 270.0)],
        },
        Mirrored {
            horizontal: &[0.0, 90.0, 180.0],
            symmetry: Symmetry::Bilateral,
            pairs: &[(30.0, 330.0), (150.0, 210.0)],
        },
        Mirrored {
            horizontal: &[90.0, 180.0, 270.0],
            symmetry: Symmetry::Transverse,
            pairs: &[(60.0, 120.0), (300.0, 240.0), (0.0, 180.0)],
        },
        Mirrored {
            horizontal: &[0.0, 120.0, 240.0, 360.0],
            symmetry: Symmetry::Full,
            pairs: &[(-30.0, 330.0), (400.0, 40.0)],
        },
    ];
    for Mirrored {
        horizontal,
        symmetry,
        pairs,
    } in cases
    {
        let profile = parsed(&profile("IESNA:LM-63-2002", 1.0, horizontal));
        assert_eq!(profile.symmetry(), symmetry);
        for &(a, b) in pairs {
            assert_eq!(
                profile.candela(0.0, a),
                profile.candela(0.0, b),
                "{symmetry:?}: {a}° mirrors {b}°"
            );
        }
    }
    let full = parsed(&profile(
        "IESNA:LM-63-2002",
        1.0,
        &[0.0, 120.0, 240.0, 360.0],
    ));
    assert!((full.candela(0.0, 60.0) - 160.0).abs() < 1e-3);
}

#[test]
fn lambertian_profile_integrates_to_pi_times_its_peak() {
    let vertical: Vec<f32> = (0..=36).map(|step| step as f32 * 2.5).collect();
    let values: Vec<String> = vertical
        .iter()
        .map(|angle| (1000.0 * angle.to_radians().cos()).max(0.0).to_string())
        .collect();
    let angles: Vec<String> = vertical.iter().map(f32::to_string).collect();
    let text = format!(
        "IES:LM-63-2019\nTILT=NONE\n1 -1 1 {} 1 1 2 0 0 0\n1 1 10\n{}\n0\n{}\n",
        vertical.len(),
        angles.join(" "),
        values.join(" ")
    );
    let profile = parsed(&text);
    let steps = 7200;
    let step = PI / steps as f32;
    let flux: f32 = (0..steps)
        .map(|index| {
            let theta = (index as f32 + 0.5) * step;
            profile.candela(theta.to_degrees(), 0.0) * theta.sin() * step
        })
        .sum::<f32>()
        * 2.0
        * PI;
    let expected = PI * 1000.0;
    assert!(
        (flux - expected).abs() < expected * 2e-3,
        "flux {flux} lm, expected {expected}"
    );
}

#[test]
fn latin1_keywords_are_read() {
    let mut bytes = b"IESNA:LM-63-2002\n[OTHER] 45".to_vec();
    bytes.push(0xB0);
    bytes.extend_from_slice(b"\nTILT=NONE\n1 -1 1 2 1 1 2 0 0 0\n1 1 1\n0 90\n0\n5 0\n");
    let profile = IesProfile::parse(&bytes).expect("latin-1 profile");
    assert_eq!(profile.keyword("OTHER"), Some("45°"));
}

#[test]
fn malformed_profiles_are_refused() {
    let base = "IESNA:LM-63-2002\nTILT=NONE\n1 -1 1 3 1 1 2 0 0 0\n1 1 1\n0 45 90\n0\n";
    let cases = [
        ("IESNA:LM-63-2002\n[TEST] x\n", IesError::MissingTilt),
        (
            "IESNA:LM-63-2002\nTILT=INCLUDE\n",
            IesError::Tilt("INCLUDE".into()),
        ),
        (
            "IESNA:LM-63-2002\nTILT=NONE\n1 -1 1 3 1 2 2 0 0 0\n1 1 1\n",
            IesError::PhotometricType(2),
        ),
        (
            &format!("{base}10 5\n"),
            IesError::Truncated("candela values"),
        ),
        (
            "IESNA:LM-63-2002\nTILT=NONE\n1 -1 1 3 1 1 2 0 0 0\n1 1 1\n0 90 45\n0\n1 1 1\n",
            IesError::Angles {
                what: "vertical",
                reason: "not ascending",
            },
        ),
        (
            "IESNA:LM-63-2002\nTILT=NONE\n1 -1 1 2 2 1 2 0 0 0\n1 1 1\n0 90\n0 120\n1 1 1 1\n",
            IesError::Angles {
                what: "horizontal",
                reason: "type C runs 0–90, 0–180, 0–360 or 90–270",
            },
        ),
        (
            &format!("{base}10 five 0\n"),
            IesError::Number {
                what: "candela values",
                token: "five".into(),
            },
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(
            IesProfile::parse(text.as_bytes()),
            Err(expected.clone()),
            "{text}"
        );
    }
}

#[test]
fn angle_counts_whose_product_overflows_32_bits_are_refused() {
    const COUNT: u32 = 1 << 16;
    let spread = |span: f32| {
        (0..COUNT)
            .map(|index| (span * index as f32 / (COUNT - 1) as f32).to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let text = format!(
        "IESNA:LM-63-2002\nTILT=NONE\n1 -1 1 {COUNT} {COUNT} 1 2 0 0 0\n1 1 1\n{}\n{}\n1 2 3\n",
        spread(180.0),
        spread(360.0)
    );
    assert_eq!(
        IesProfile::parse(text.as_bytes()),
        Err(IesError::Truncated("candela values"))
    );
}
