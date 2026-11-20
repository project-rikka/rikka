use rikka_mesh_bevel::{BevelOptions, Error, WidthType, bevel_selected};

use crate::support::build_case;

#[test]
fn invalid_options_are_rejected() {
    let cases = [
        BevelOptions {
            width: -0.1,
            ..BevelOptions::default()
        },
        BevelOptions {
            width: f32::NAN,
            ..BevelOptions::default()
        },
        BevelOptions {
            width_type: WidthType::Percent,
            width: 120.0,
            ..BevelOptions::default()
        },
        BevelOptions {
            width: 0.1,
            segments: 0,
            ..BevelOptions::default()
        },
        BevelOptions {
            width: 0.1,
            profile_shape: 1.5,
            ..BevelOptions::default()
        },
        BevelOptions {
            width: 0.1,
            spread: -1.0,
            ..BevelOptions::default()
        },
    ];
    for options in &cases {
        let mut mesh = build_case("CUBE_EDGE");
        let result = bevel_selected(&mut mesh, options);
        assert!(
            matches!(
                result,
                Err(Error::InvalidWidth(_)
                    | Error::InvalidPercent(_)
                    | Error::InvalidSegments(_)
                    | Error::InvalidProfileShape(_)
                    | Error::InvalidSpread(_))
            ),
            "{options:?} was accepted"
        );
        assert_eq!(mesh.face_count(), 6);
    }
}
