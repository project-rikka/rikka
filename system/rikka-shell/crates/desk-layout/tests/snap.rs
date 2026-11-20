use desk_layout::{GAP, Guide, Snapper};
use rikka_types::Rect;

const ROOM: Rect = Rect::new(0.0, 0.0, 1000.0, 800.0);

struct Case<'a> {
    name: &'a str,
    others: &'a [Rect],
    moving: Rect,
    lands: (f32, f32),
}

#[test]
fn rects_snap_to_nearest_guide() {
    let left = Rect::new(100.0, 100.0, 200.0, 100.0);
    let right = Rect::new(700.0, 100.0, 200.0, 100.0);
    let below = Rect::new(100.0, 400.0, 200.0, 100.0);
    let cases = [
        Case {
            name: "room edges",
            others: &[],
            moving: Rect::new(4.0, 795.0 - 100.0, 50.0, 100.0),
            lands: (0.0, 700.0),
        },
        Case {
            name: "room centre",
            others: &[],
            moving: Rect::new(470.0, 300.0, 50.0, 50.0),
            lands: (475.0, 300.0),
        },
        Case {
            name: "aligned left edges",
            others: &[left],
            moving: Rect::new(106.0, 300.0, 80.0, 40.0),
            lands: (100.0, 300.0),
        },
        Case {
            name: "standard gap after a neighbour",
            others: &[left],
            moving: Rect::new(300.0 + GAP + 5.0, 130.0, 80.0, 40.0),
            lands: (300.0 + GAP, 130.0),
        },
        Case {
            name: "equal gaps between two neighbours",
            others: &[left, right],
            moving: Rect::new(445.0, 120.0, 100.0, 60.0),
            lands: (450.0, 120.0),
        },
        Case {
            name: "repeating an existing gap",
            others: &[left, below],
            moving: Rect::new(140.0, 400.0 + 100.0 + 200.0 - 3.0, 60.0, 50.0),
            lands: (140.0, 700.0),
        },
        Case {
            name: "nothing within reach",
            others: &[left],
            moving: Rect::new(230.0, 260.0, 30.0, 30.0),
            lands: (230.0, 260.0),
        },
    ];
    for case in cases {
        let snapped = Snapper::new(ROOM, case.others).snap(case.moving);
        let name = case.name;
        assert_eq!(
            (snapped.rect.min_x(), snapped.rect.min_y()),
            case.lands,
            "{name}"
        );
        assert_eq!(snapped.rect.size, case.moving.size, "{name} kept its size");
    }
}

#[test]
fn snap_reports_matched_guides() {
    let left = Rect::new(100.0, 100.0, 200.0, 100.0);
    let snapped = Snapper::new(ROOM, &[left]).snap(Rect::new(103.0, 400.0, 200.0, 60.0));
    assert!(snapped.guides.contains(&Guide::Vertical {
        x: 100.0,
        top: 100.0,
        bottom: 460.0
    }));
    assert!(snapped.guides.contains(&Guide::Vertical {
        x: 300.0,
        top: 100.0,
        bottom: 460.0
    }));

    let beside = Snapper::new(ROOM, &[left]).snap(Rect::new(318.0, 120.0, 50.0, 50.0));
    assert!(beside.guides.iter().any(|guide| matches!(
        guide,
        Guide::Spacing { span, across_x: true } if span.width() == GAP && span.min_x() == 300.0
    )));
}

#[test]
fn guides_require_exact_match() {
    let others = [Rect::new(200.0, 200.0, 100.0, 100.0)];
    let snapper = Snapper::new(ROOM, &others);
    assert!(
        snapper
            .matches(Rect::new(213.0, 513.0, 50.0, 50.0))
            .is_empty()
    );
    assert!(
        !snapper
            .matches(Rect::new(200.0, 513.0, 50.0, 50.0))
            .is_empty()
    );
}
