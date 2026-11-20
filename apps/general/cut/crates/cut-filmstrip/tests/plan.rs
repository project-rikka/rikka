use cut_filmstrip::{Footage, Mode, Tile, Window, tiles};

const THUMB: f32 = 80.0;

fn footage(start: i64, end: i64, source_in: i64, speed: f64) -> Footage {
    Footage {
        start,
        end,
        source_in,
        speed,
        last: 100_000,
    }
}

fn window(first: f64, per_frame: f64) -> Window {
    Window {
        first,
        per_frame,
        width: 1200.0,
    }
}

fn assert_covers(planned: &[Tile], from: f32, to: f32) {
    assert!(!planned.is_empty());
    assert!(planned[0].left <= from + 0.01, "{planned:?} starts late");
    for pair in planned.windows(2) {
        assert!(
            (pair[0].right - pair[1].left).abs() < 0.01,
            "a gap in {planned:?}"
        );
        assert!(
            pair[0].source < pair[1].source,
            "{planned:?} goes backwards"
        );
    }
    assert!(
        planned[planned.len() - 1].right >= to - 0.01,
        "{planned:?} ends early"
    );
}

#[test]
fn continuous_strip_covers_view_without_gaps() {
    for (clip, view) in [
        (footage(10, 400, 37, 1.0), window(0.0, 2.0)),
        (footage(10, 400, 37, 1.0), window(100.0, 2.0)),
        (footage(0, 90, 0, 1.0), window(0.0, 48.0)),
        (footage(0, 5000, 5, 2.5), window(0.0, 0.25)),
        (footage(200, 260, 999, 0.1), window(150.0, 9.0)),
    ] {
        let planned = tiles(Mode::Continuous, clip, view, THUMB);
        let left = (((clip.start as f64 - view.first) * view.per_frame) as f32).max(0.0);
        let right = (((clip.end as f64 - view.first) * view.per_frame) as f32).min(view.width);
        assert_covers(&planned, left, right);
        let last = clip.source_in + ((clip.end - clip.start) as f64 * clip.speed).ceil() as i64;
        let widest = THUMB.max((view.per_frame / clip.speed) as f32);
        for tile in &planned {
            assert!(
                tile.right - tile.left <= widest + 0.01,
                "{tile:?} is too wide"
            );
            assert!(
                (clip.source_in..last).contains(&tile.source),
                "{tile:?} lies outside the clip"
            );
            assert!(
                tile.source + tile.tolerance <= last,
                "{tile:?} reaches past the clip"
            );
        }
        if clip.start as f64 >= view.first {
            assert_eq!(planned[0].source, clip.source_in);
            assert_eq!(planned[0].tolerance, 0);
        }
    }
}

#[test]
fn zooming_in_keeps_coarser_frames() {
    let clip = footage(0, 4000, 0, 1.0);
    let coarse = tiles(Mode::Continuous, clip, window(0.0, 0.5), THUMB);
    let finer = tiles(Mode::Continuous, clip, window(0.0, 1.0), THUMB);
    let slightly = tiles(Mode::Continuous, clip, window(0.0, 0.55), THUMB);

    let sources = |planned: &[Tile]| planned.iter().map(|tile| tile.source).collect::<Vec<_>>();
    let finer_sources = sources(&finer);
    for tile in coarse.iter().filter(|tile| tile.right <= 600.0) {
        assert!(
            finer_sources.contains(&tile.source),
            "{tile:?} is gone when zoomed in"
        );
    }
    assert_eq!(sources(&slightly)[..10], sources(&coarse)[..10]);
}

#[test]
fn head_and_tail_show_end_frames() {
    let clip = footage(0, 300, 20, 1.0);
    let view = window(0.0, 2.0);

    let head = tiles(Mode::Head, clip, view, THUMB);
    assert_eq!(head.len(), 1);
    assert_eq!(
        (head[0].left, head[0].right, head[0].source),
        (0.0, THUMB, 20)
    );

    let ends = tiles(Mode::HeadAndTail, clip, view, THUMB);
    assert_eq!(ends.len(), 2);
    assert_eq!(ends[1].source, 319);
    assert!(ends[1].tail && ends[1].right == 600.0 && ends[1].left == 600.0 - THUMB);

    let short = footage(0, 60, 20, 1.0);
    assert_eq!(tiles(Mode::HeadAndTail, short, view, THUMB).len(), 1);
    assert!(tiles(Mode::Off, clip, view, THUMB).is_empty());
    assert!(tiles(Mode::Continuous, clip, window(400.0, 2.0), THUMB).is_empty());
}

#[test]
fn frames_past_media_repeat_last_frame() {
    let clip = Footage {
        last: 50,
        ..footage(0, 200, 0, 1.0)
    };
    let planned = tiles(Mode::Continuous, clip, window(0.0, 4.0), THUMB);

    assert!(
        planned
            .iter()
            .all(|tile| tile.source + tile.tolerance <= 51)
    );
    assert!(planned.iter().filter(|tile| tile.source == 50).count() > 1);
}
