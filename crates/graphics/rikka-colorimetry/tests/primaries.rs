use rikka_colorimetry::{Chromaticity, Matrix3, Primaries, bradford};

fn assert_matrix(actual: Matrix3, expected: [[f64; 3]; 3], tolerance: f64, what: &str) {
    for (row, (actual, expected)) in actual.rows().iter().zip(expected).enumerate() {
        for column in 0..3 {
            assert!(
                (actual[column] - expected[column]).abs() <= tolerance,
                "{what}[{row}][{column}] is {}, not {}",
                actual[column],
                expected[column]
            );
        }
    }
}

#[test]
fn srgb_primaries_give_the_iec_matrix() {
    assert_matrix(
        Primaries::SRGB.to_xyz().unwrap(),
        [
            [0.4124, 0.3576, 0.1805],
            [0.2126, 0.7152, 0.0722],
            [0.0193, 0.1192, 0.9505],
        ],
        2.0e-4,
        "sRGB to XYZ",
    );
}

#[test]
fn bt709_to_bt2020_matches_bt2087() {
    let into = Primaries::SRGB.to_xyz().unwrap();
    let from = Primaries::BT2020.to_xyz().unwrap().inverse().unwrap();
    assert_matrix(
        from * into,
        [
            [0.6274, 0.3293, 0.0433],
            [0.0691, 0.9195, 0.0114],
            [0.0164, 0.0880, 0.8956],
        ],
        1.0e-4,
        "BT.709 to BT.2020",
    );
}

#[test]
fn bradford_adapts_d65_to_d50() {
    assert_matrix(
        bradford(Chromaticity::D65, Chromaticity::D50),
        [
            [1.0478, 0.0229, -0.0501],
            [0.0295, 0.9905, -0.0170],
            [-0.0092, 0.0150, 0.7519],
        ],
        1.0e-3,
        "D65 to D50",
    );
    assert!(bradford(Chromaticity::D65, Chromaticity::D65).is_identity(0.0));
}

#[test]
fn cie_xyz_primaries_are_the_identity() {
    assert!(Primaries::CIE1931_XYZ.to_xyz().unwrap().is_identity(1.0e-9));
}

#[test]
fn degenerate_primaries_are_invalid() {
    let collinear = Primaries::new(
        Chromaticity::new(0.2, 0.2),
        Chromaticity::new(0.3, 0.3),
        Chromaticity::new(0.4, 0.4),
        Chromaticity::D65,
    );
    assert!(!collinear.is_valid());
    assert!(Primaries::DISPLAY_P3.is_valid());
}
