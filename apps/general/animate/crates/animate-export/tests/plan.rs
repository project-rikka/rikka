use animate_export::{FRAME_RATES, Plan};

const TIMELINE_RATE: f64 = 30.0;
const SAMPLE_RATES: [u32; 2] = [44_100, 48_000];
const RANGES: [(u32, u32); 3] = [(0, 0), (0, 299), (45, 1234)];

#[test]
fn sound_samples_tile_the_exported_span() {
    for (rate, name) in FRAME_RATES {
        for (first, last) in RANGES {
            let plan = Plan::new(first, last, rate);
            for sample_rate in SAMPLE_RATES {
                let spans: Vec<_> = (0..plan.frames())
                    .map(|index| plan.samples(index, sample_rate))
                    .collect();
                assert!(
                    spans.windows(2).all(|pair| pair[0].end == pair[1].start),
                    "{name} fps leaves a gap or overlap"
                );
                let start = u64::from(first) * u64::from(sample_rate) / 30;
                assert_eq!(spans[0].start, start, "{name} fps starts late");
                let covered = spans[spans.len() - 1].end - start;
                let expected = plan.duration().as_secs_f64() * f64::from(sample_rate);
                assert!(
                    (covered as f64 - expected).abs() <= 1.0,
                    "{name} fps covers {covered} of {expected} samples"
                );
            }
        }
    }
}

#[test]
fn output_frames_step_evenly_through_the_timeline_range() {
    for (rate, name) in FRAME_RATES {
        let plan = Plan::new(30, 329, rate);
        let per_second = f64::from(rate.numerator()) / f64::from(rate.denominator());
        assert_eq!(
            plan.frames(),
            (10.0 * per_second).round() as u64,
            "{name} fps"
        );
        assert_eq!(plan.time(0), 1.0, "{name} fps");
        for index in 1..plan.frames() {
            let step = f64::from(plan.time(index) - plan.time(index - 1));
            assert!(
                (step - 1.0 / per_second).abs() < 1e-4,
                "{name} fps at {index}"
            );
        }
        let last = f64::from(plan.time(plan.frames() - 1));
        assert!(
            last < 330.0 / TIMELINE_RATE,
            "{name} fps runs past the range"
        );
    }
}
