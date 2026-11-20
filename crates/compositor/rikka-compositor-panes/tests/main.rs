use rikka_blur::rectangles;
use rikka_compositor_geometry::{Logical, Point, Rect, Region, Size};
use rikka_compositor_panes::read_panes;
use rikka_types::{CornerRadii, Rect as SceneRect, RoundedRect};

fn surface() -> Size<i32, Logical> {
    Size::new(800, 600)
}

fn pane(x: f32, y: f32, width: f32, height: f32, radius: f32) -> RoundedRect {
    RoundedRect::new(
        SceneRect::new(x, y, width, height),
        CornerRadii::uniform(radius),
    )
}

fn drawn(panes: &[RoundedRect]) -> Region<Logical> {
    let mut steps = Vec::new();
    for pane in panes {
        rectangles(*pane, &mut steps);
    }
    steps
        .into_iter()
        .map(|[x, y, width, height]| Rect::new(Point::new(x, y), Size::new(width, height)))
        .collect()
}

fn read(region: &Region<Logical>) -> Vec<RoundedRect> {
    let mut panes = Vec::new();
    read_panes(region, surface(), |pane| panes.push(pane));
    panes
}

fn area(region: &Region<Logical>) -> i64 {
    region
        .iter()
        .map(|step| i64::from(step.size.w) * i64::from(step.size.h))
        .sum()
}

#[test]
fn lone_pane_reads_back_as_drawn() {
    for original in [
        pane(40.0, 30.0, 320.0, 200.0, 0.0),
        pane(40.0, 30.0, 320.0, 200.0, 12.0),
        pane(0.0, 0.0, 800.0, 600.0, 24.0),
        pane(100.0, 100.0, 240.0, 48.0, 24.0),
        pane(300.0, 200.0, 96.0, 96.0, 48.0),
        pane(10.0, 500.0, 300.0, 64.0, 6.0),
    ] {
        assert_eq!(read(&drawn(&[original])), vec![original], "{original:?}");
    }
}

#[test]
fn neighbouring_panes_never_frost_outside_their_region() {
    let scenes: [&[RoundedRect]; 4] = [
        &[
            pane(40.0, 40.0, 300.0, 400.0, 16.0),
            pane(340.0, 80.0, 200.0, 240.0, 16.0),
        ],
        &[
            pane(40.0, 40.0, 300.0, 200.0, 12.0),
            pane(40.0, 260.0, 300.0, 200.0, 12.0),
            pane(400.0, 40.0, 64.0, 64.0, 32.0),
        ],
        &[
            pane(100.0, 100.0, 400.0, 300.0, 20.0),
            pane(300.0, 250.0, 400.0, 300.0, 20.0),
        ],
        &[
            pane(0.0, 0.0, 800.0, 48.0, 0.0),
            pane(16.0, 64.0, 200.0, 500.0, 18.0),
            pane(232.0, 64.0, 552.0, 500.0, 18.0),
        ],
    ];
    for scene in scenes {
        let region = drawn(scene);
        let panes = read(&region);
        assert!(panes.len() >= scene.len().min(2) && panes.len() <= 16);
        let mut outside = drawn(&panes);
        outside.subtract_region(&region);
        assert_eq!(area(&outside), 0, "{panes:?} frost outside {scene:?}");
        let mut missed = region.clone();
        missed.subtract_region(&drawn(&panes));
        assert!(
            area(&missed) * 50 <= area(&region),
            "{panes:?} leave {} of {} uncovered",
            area(&missed),
            area(&region)
        );
    }
}

#[test]
fn region_is_cut_to_the_surface_and_empty_regions_have_no_pane() {
    let hanging = drawn(&[pane(700.0, 500.0, 300.0, 300.0, 0.0)]);
    assert_eq!(read(&hanging), vec![pane(700.0, 500.0, 100.0, 100.0, 0.0)]);
    assert!(read(&Region::default()).is_empty());
}
