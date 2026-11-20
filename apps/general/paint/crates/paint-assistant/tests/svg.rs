use paint_assistant::{outlines, path};

fn heights(d: &str) -> (f32, f32) {
    outlines(&path(d).expect("a path"))
        .iter()
        .flatten()
        .fold((f32::MAX, f32::MIN), |(low, high), point| {
            (low.min(point.1), high.max(point.1))
        })
}

#[test]
fn arcs_convert_to_curves() {
    assert!((heights("M0 0 A50 50 0 0 1 100 0").0 + 50.0).abs() < 0.5);
    assert!((heights("M0 0 a50 50 0 0 0 100 0").1 - 50.0).abs() < 0.5);
    assert!((heights("M0 0 A1 1 0 0 1 100 0").0 + 50.0).abs() < 0.5);
    let packed = outlines(&path("M0 0a50 50 0 1050 50").expect("a path"));
    let last = packed[0].last().expect("an end");
    assert!((last.0 - 50.0).abs() < 1e-3 && (last.1 - 50.0).abs() < 1e-3);
}

#[test]
fn path_builds_closed_subpaths() {
    let built = path("M0 0 H10 V10 Z M20 0 C20 10 30 10 30 0").expect("a path");
    assert_eq!(built.subpaths().len(), 2);
    assert!(built.subpaths()[0].is_closed());
    assert!(!built.subpaths()[1].is_closed());
    assert!(built.subpaths()[1].points().len() > 4);
    let deepest = outlines(&built)[1]
        .iter()
        .map(|point| point.1)
        .fold(f32::MIN, f32::max);
    assert!((deepest - 7.5).abs() < 0.3, "{deepest}");
    assert!(path("M5 5").is_err());
}

#[test]
fn parse_errors_report_position() {
    let cases = [
        ("M0 0 L10", "numbers"),
        ("M0 0 X10 10", "character 6: 'X'"),
        ("L10 10", "starts with M"),
        ("M0 0 A5 5 0 2 0 10 10", "0 or 1"),
        ("M0 0 L1e999 0", "too large"),
        ("M0 0 L1 x", "found 'x'"),
        ("  ", "empty"),
    ];
    for (d, says) in cases {
        let refusal = path(d).expect_err(d);
        assert!(refusal.contains(says), "{d:?}: {refusal}");
    }
}
