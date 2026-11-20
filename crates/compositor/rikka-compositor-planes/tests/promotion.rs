use std::time::Duration;

use rikka_compositor_planes::{Assignment, Promotion};

use crate::assign::scanned_out;
struct Step {
    at: u64,
    candidate: Option<u32>,
    generation: u64,
    placed: Option<u32>,
}

const fn step(at: u64, candidate: Option<u32>, generation: u64, placed: Option<u32>) -> Step {
    Step {
        at,
        candidate,
        generation,
        placed,
    }
}

#[test]
fn promotion_requires_steady_updates() {
    let cases: [(&str, &[Step]); 5] = [
        (
            "an updating surface is placed once it settled and then stays",
            &[
                step(0, Some(1), 0, None),
                step(50, Some(1), 3, None),
                step(100, Some(1), 6, Some(1)),
                step(105, Some(1), 6, Some(1)),
                step(2000, Some(1), 6, Some(1)),
            ],
        ),
        (
            "a surface that shows no new buffers stays composited",
            &[
                step(0, Some(1), 4, None),
                step(200, Some(1), 4, None),
                step(800, Some(1), 5, None),
            ],
        ),
        (
            "a break in candidacy starts the wait over",
            &[
                step(0, Some(1), 0, None),
                step(60, Some(1), 2, None),
                step(70, None, 3, None),
                step(80, Some(1), 4, None),
                step(170, Some(1), 8, None),
                step(180, Some(1), 9, Some(1)),
            ],
        ),
        (
            "a placed surface leaves at once and settles again before it returns",
            &[
                step(0, Some(1), 0, None),
                step(100, Some(1), 4, Some(1)),
                step(110, None, 5, None),
                step(120, Some(1), 6, None),
                step(220, Some(1), 9, Some(1)),
            ],
        ),
        (
            "another surface takes the plane only once it settled",
            &[
                step(0, Some(1), 0, None),
                step(100, Some(1), 4, Some(1)),
                step(110, Some(2), 0, None),
                step(150, Some(2), 3, None),
                step(210, Some(2), 6, Some(2)),
            ],
        ),
    ];
    for (case, steps) in cases {
        let mut promotion = Promotion::default();
        for (index, step) in steps.iter().enumerate() {
            let assignment = match step.candidate {
                Some(surface) => scanned_out(surface),
                None => Assignment::Composited(None),
            };
            let settled = promotion.update(
                assignment,
                Duration::from_millis(step.at),
                Some(step.generation),
            );
            let placed = settled.layout().map(|layout| layout.surface);
            assert_eq!(placed, step.placed, "{case}: step {index}");
        }
    }
}
