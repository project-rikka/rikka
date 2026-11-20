use rikka_path_data::{Error, ErrorKind, Segment, segments};
use rikka_types::Point;

fn read(d: &str) -> Result<Vec<Segment>, Error> {
    segments(d).collect()
}

fn at(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

#[test]
fn commands_resolve_to_absolute_segments() {
    let cases: [(&str, Vec<Segment>); 8] = [
        ("", Vec::new()),
        (
            "M10,20 L30 40 z",
            vec![
                Segment::MoveTo(at(10.0, 20.0)),
                Segment::LineTo(at(30.0, 40.0)),
                Segment::Close,
            ],
        ),
        (
            "m1 1 2 2 h3v-1",
            vec![
                Segment::MoveTo(at(1.0, 1.0)),
                Segment::LineTo(at(3.0, 3.0)),
                Segment::LineTo(at(6.0, 3.0)),
                Segment::LineTo(at(6.0, 2.0)),
            ],
        ),
        (
            "M0 0C1 0 2 1 2 2S3 4 4 4",
            vec![
                Segment::MoveTo(at(0.0, 0.0)),
                Segment::CubicTo(at(1.0, 0.0), at(2.0, 1.0), at(2.0, 2.0)),
                Segment::CubicTo(at(2.0, 3.0), at(3.0, 4.0), at(4.0, 4.0)),
            ],
        ),
        (
            "M0 0Q1 1 2 0T4 0",
            vec![
                Segment::MoveTo(at(0.0, 0.0)),
                Segment::QuadTo(at(1.0, 1.0), at(2.0, 0.0)),
                Segment::QuadTo(at(3.0, -1.0), at(4.0, 0.0)),
            ],
        ),
        (
            "M0 0S1 1 2 2",
            vec![
                Segment::MoveTo(at(0.0, 0.0)),
                Segment::CubicTo(at(0.0, 0.0), at(1.0, 1.0), at(2.0, 2.0)),
            ],
        ),
        (
            "M-.5.5e1 1e-1-2",
            vec![
                Segment::MoveTo(at(-0.5, 5.0)),
                Segment::LineTo(at(0.1, -2.0)),
            ],
        ),
        (
            "M0 0 A0 5 0 0 1 4 0",
            vec![Segment::MoveTo(at(0.0, 0.0)), Segment::LineTo(at(4.0, 0.0))],
        ),
    ];
    for (d, expected) in cases {
        assert_eq!(read(d), Ok(expected), "{d:?}");
    }
}

#[test]
fn arcs_become_cubics_ending_at_the_target() {
    let read = read("M0 0 a5 5 0 1 1 10 0").expect("an arc");
    assert_eq!(read.len(), 3);
    let Some(Segment::CubicTo(_, _, end)) = read.last() else {
        panic!("{read:?} ends in a cubic");
    };
    assert_eq!(*end, at(10.0, 0.0));
    for segment in &read[1..] {
        let Segment::CubicTo(_, _, through) = segment else {
            panic!("{segment:?} is a cubic");
        };
        let radius = (through.x - 5.0).hypot(through.y);
        assert!((radius - 5.0).abs() < 1e-3, "{through:?}");
    }
}

#[test]
fn malformed_data_names_its_offset() {
    let cases = [
        ("L1 1", 0, ErrorKind::NoMoveFirst),
        ("  10 10", 2, ErrorKind::NoMoveFirst),
        ("M1 1 L2", 7, ErrorKind::UnexpectedEnd),
        ("M1 1 L2 x", 8, ErrorKind::Number),
        ("M1 1 X2 2", 5, ErrorKind::UnknownCommand('X')),
        ("M0 0 A1 1 0 2 0 3 3", 12, ErrorKind::Flag),
        ("M0 0 Z 1 1", 7, ErrorKind::UnknownCommand('1')),
    ];
    for (d, offset, kind) in cases {
        assert_eq!(read(d), Err(Error { offset, kind }), "{d:?}");
    }
}

#[test]
fn reading_stops_after_the_first_error() {
    let mut read = segments("M1 1 L2 2 L# 3 3 L4 4");
    assert!(matches!(read.next(), Some(Ok(Segment::MoveTo(_)))));
    assert!(matches!(read.next(), Some(Ok(Segment::LineTo(_)))));
    assert!(matches!(read.next(), Some(Err(_))));
    assert_eq!(read.next(), None);
}
