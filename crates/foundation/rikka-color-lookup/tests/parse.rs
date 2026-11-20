use std::fmt::Write;

use rikka_color_lookup::{ColorLookup, Domain, ParseError};

fn swapped_cube(header: &str) -> String {
    let mut text = String::from(header);
    for blue in 0..2 {
        for green in 0..2 {
            for red in 0..2 {
                writeln!(text, "{blue}.0 {green} {red}").expect("write to String");
            }
        }
    }
    text
}

#[test]
fn cube_files_follow_the_adobe_and_resolve_keywords() {
    let cases: [(&str, Domain); 4] = [
        ("LUT_3D_SIZE 2\n", Domain::default()),
        (
            "TITLE \"swap\"\n# made by hand\nLUT_3D_SIZE 2 # lattice\nLUT_IN_VIDEO_RANGE\n",
            Domain::default(),
        ),
        (
            "LUT_3D_SIZE 2\nDOMAIN_MIN 0 0 -0.5\nDOMAIN_MAX 1 2 1\n",
            Domain {
                min: [0.0, 0.0, -0.5],
                max: [1.0, 2.0, 1.0],
            },
        ),
        (
            "LUT_3D_SIZE 2\nLUT_3D_INPUT_RANGE -0.25 4\n",
            Domain {
                min: [-0.25; 3],
                max: [4.0; 3],
            },
        ),
    ];
    for (header, domain) in cases {
        let lookup = ColorLookup::parse(&swapped_cube(header)).expect(header);
        assert_eq!(lookup.size(), 2, "{header}");
        assert_eq!(lookup.domain(), domain, "{header}");
        assert_eq!(lookup.entry(1, 0, 0), [0.0, 0.0, 1.0], "{header}");
        assert_eq!(lookup.entry(0, 1, 1), [1.0, 1.0, 0.0], "{header}");
        assert_eq!(lookup.table().len(), 2 * 2 * 2 * 3);
    }
}

#[test]
fn three_dl_rescales_integer_tables_into_red_fastest_order() {
    let mut text = String::from("3DMESH\nMesh 2 10\n0 341 682 1023\n");
    for red in 0..4 {
        for green in 0..4 {
            for blue in 0..4 {
                writeln!(text, "{} {} {}", red * 341, green * 341, blue * 341)
                    .expect("write to String");
            }
        }
    }
    let lookup = ColorLookup::parse(&text).expect("a valid 3dl table");
    assert_eq!(lookup.size(), 4);
    assert_eq!(lookup.entry(3, 0, 0), [1.0, 0.0, 0.0]);
    assert_eq!(lookup.entry(0, 3, 3), [0.0, 1.0, 1.0]);
}

#[test]
fn malformed_tables_are_refused() {
    let cases: [(&str, ParseError); 6] = [
        ("hello", ParseError::Unrecognized),
        (
            "LUT_3D_SIZE 2\n0 0 0\n",
            ParseError::Entries {
                size: 2,
                expected: 8,
                found: 1,
            },
        ),
        ("LUT_1D_SIZE 16\n", ParseError::OneDimensional),
        ("LUT_3D_SIZE 1\n0 0 0\n", ParseError::Size(1)),
        (
            "LUT_3D_SIZE 2\n0 0\n",
            ParseError::Line {
                line: 2,
                reason: "expected three numbers",
            },
        ),
        (
            "LUT_3D_SIZE 2\nDOMAIN_MIN 1 0 0\nDOMAIN_MAX 1 1 1\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n",
            ParseError::Domain,
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(ColorLookup::parse(text), Err(expected), "{text}");
    }
}
