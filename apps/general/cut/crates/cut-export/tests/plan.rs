use cut_export::{Plan, Span};
use cut_project::Rate;

#[test]
fn output_frames_sample_span_and_tile_sound() {
    let cases = [
        (Rate::VIDEO, Rate::VIDEO, 90, 90, [(0, 0), (1, 1), (89, 89)]),
        (
            Rate::VIDEO,
            Rate::HIGH,
            90,
            180,
            [(1, 0), (2, 1), (179, 89)],
        ),
        (Rate::HIGH, Rate::VIDEO, 90, 45, [(1, 2), (2, 4), (44, 88)]),
        (
            Rate::NTSC_VIDEO,
            Rate::VIDEO,
            1001,
            1002,
            [(1, 1), (500, 500), (1001, 1000)],
        ),
        (
            Rate::VIDEO,
            Rate::NTSC_FILM,
            30,
            24,
            [(1, 1), (12, 15), (23, 29)],
        ),
    ];
    for (timeline, output, length, frames, landmarks) in cases {
        let first = 40;
        let plan = Plan::new(
            Span::new(first, first + length).expect("a span"),
            timeline,
            output,
        );
        assert_eq!(plan.frames(), frames, "{timeline} → {output}");
        for (index, offset) in landmarks {
            assert_eq!(
                plan.frame(index),
                first + offset,
                "{timeline} → {output} #{index}"
            );
        }
        let mut previous = first;
        let mut covered = 0;
        for index in 0..plan.frames() {
            let frame = plan.frame(index);
            assert!(frame >= previous && frame < first + length);
            previous = frame;
            let samples = plan.samples(index, 48_000);
            assert_eq!(samples.start, covered, "{timeline} → {output} #{index}");
            covered = samples.end;
        }
        let expected = plan.duration().as_secs_f64() * 48_000.0;
        assert!(
            (covered as f64 - expected).abs() < 1.0,
            "{covered} vs {expected}"
        );
    }
}
