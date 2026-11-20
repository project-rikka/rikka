use cut_project::{
    Edge, Edit, EditPoint, Error, Marker, MarkerChange, MarkerScope, Project, Switch,
};

use crate::support::{applied, clip, clips_on, film, imported};

fn spans(project: &Project, layer: usize) -> Vec<(i64, i64)> {
    clips_on(project, layer)
        .iter()
        .map(|clip| (clip.start(), clip.end()))
        .collect()
}

fn handled() -> (Project, Vec<cut_project::ClipId>) {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4)]);
    let project = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[0]],
            edge: Edge::Tail,
            by: -20,
        },
    );
    let project = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[1]],
            edge: Edge::Head,
            by: 20,
        },
    );
    let project = applied(
        &project,
        Edit::Move {
            clips: vec![ids[1]],
            by: -40,
            across: 0,
        },
    );
    assert_eq!(spans(&project, 0), vec![(0, 80), (80, 160)]);
    (project, ids)
}

#[test]
fn roll_moves_edit_within_handles() {
    let (project, ids) = handled();
    let edit_point = EditPoint::head(ids[1]);
    assert_eq!(project.roll_limits(edit_point), -20..=20);

    let rolled = applied(&project, Edit::Roll { edit_point, by: 10 });

    assert_eq!(spans(&rolled, 0), vec![(0, 90), (90, 160)]);
    assert_eq!(clip(&rolled, ids[1]).source_in(), 30);
    assert_eq!(clip(&rolled, ids[0]).source_at(89), 89);
    assert!(matches!(
        project.apply(&Edit::Roll { edit_point, by: 21 }),
        Err(Error::OutOfReach)
    ));
}

#[test]
fn roll_beside_gap_trims_into_gap() {
    let (project, ids) = imported(vec![film("a", 4)]);
    let project = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[0]],
            edge: Edge::Tail,
            by: -30,
        },
    );

    let rolled = applied(
        &project,
        Edit::Roll {
            edit_point: EditPoint::tail(ids[0]),
            by: 12,
        },
    );

    assert_eq!(spans(&rolled, 0), vec![(0, 82)]);
}

#[test]
fn ripple_trim_pulls_unlocked_layers() {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4)]);
    let project = applied(
        &project,
        Edit::Place {
            media: vec![film("c", 2)],
            at: 120,
            layer: 1,
        },
    );
    let project = applied(
        &project,
        Edit::Place {
            media: vec![film("d", 2)],
            at: 120,
            layer: 2,
        },
    );
    let project = applied(
        &project,
        Edit::Set {
            layer: 2,
            switch: Switch::Locked,
            on: true,
        },
    );
    let project = applied(
        &project,
        Edit::Marker {
            on: MarkerScope::Timeline,
            change: MarkerChange::Add(Marker::new(150)),
        },
    );

    let cases = [
        (ids[0], Edge::Tail, -15, vec![(0, 85), (85, 185)]),
        (ids[1], Edge::Head, 15, vec![(0, 100), (100, 185)]),
    ];
    for (target, edge, by, first) in cases {
        let trimmed = applied(
            &project,
            Edit::RippleTrim {
                clip: target,
                edge,
                by,
            },
        );
        assert_eq!(spans(&trimmed, 0), first, "{edge:?}");
        assert_eq!(spans(&trimmed, 1), vec![(105, 155)], "{edge:?}");
        assert_eq!(spans(&trimmed, 2), vec![(120, 170)], "{edge:?}");
        let markers: Vec<i64> = trimmed
            .markers()
            .iter()
            .map(|(_, marker)| marker.at)
            .collect();
        assert_eq!(markers, vec![135], "{edge:?}");
    }
}

#[test]
fn ripple_trim_extends_and_pushes_later_clips() {
    let (project, ids) = handled();

    let trimmed = applied(
        &project,
        Edit::RippleTrim {
            clip: ids[0],
            edge: Edge::Tail,
            by: 10,
        },
    );

    assert_eq!(spans(&trimmed, 0), vec![(0, 90), (90, 170)]);
    assert!(matches!(
        project.apply(&Edit::RippleTrim {
            clip: ids[0],
            edge: Edge::Tail,
            by: 21,
        }),
        Err(Error::OutOfReach)
    ));
}

#[test]
fn ripple_trim_stops_without_room() {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4)]);
    let project = applied(
        &project,
        Edit::Place {
            media: vec![film("c", 2), film("d", 2)],
            at: 70,
            layer: 1,
        },
    );
    let project = applied(
        &project,
        Edit::Move {
            clips: vec![project.layers()[1].clips()[1].id()],
            by: 5,
            across: 0,
        },
    );
    assert_eq!(spans(&project, 1), vec![(70, 120), (125, 175)]);

    assert_eq!(project.ripple_limits(ids[1], Edge::Head), 0..=5);
    assert!(matches!(
        project.apply(&Edit::RippleTrim {
            clip: ids[1],
            edge: Edge::Head,
            by: 6,
        }),
        Err(Error::OutOfReach)
    ));
}

#[test]
fn slide_moves_neighbouring_edits() {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4), film("c", 4)]);
    let project = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[0]],
            edge: Edge::Tail,
            by: -20,
        },
    );
    let project = applied(
        &project,
        Edit::Move {
            clips: vec![ids[1]],
            by: -20,
            across: 0,
        },
    );
    let project = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[2]],
            edge: Edge::Head,
            by: 20,
        },
    );
    let project = applied(
        &project,
        Edit::Move {
            clips: vec![ids[2]],
            by: -40,
            across: 0,
        },
    );
    assert_eq!(spans(&project, 0), vec![(0, 80), (80, 180), (180, 260)]);
    assert_eq!(project.slide_limits(ids[1]), -20..=20);

    let slid = applied(
        &project,
        Edit::Slide {
            clip: ids[1],
            by: 15,
        },
    );

    assert_eq!(spans(&slid, 0), vec![(0, 95), (95, 195), (195, 260)]);
    assert_eq!(clip(&slid, ids[1]).source_in(), 0);
    assert_eq!(clip(&slid, ids[2]).source_in(), 35);
}

#[test]
fn extract_closes_gap_on_unlocked_layers() {
    let (project, _) = imported(vec![film("a", 4), film("b", 4)]);

    let extracted = applied(&project, Edit::Extract(90..110));

    assert_eq!(spans(&extracted, 0), vec![(0, 90), (90, 180)]);
    assert_eq!(extracted.end(), 180);
    assert!(matches!(
        project.apply(&Edit::Extract(10..10)),
        Err(Error::NothingToTrim)
    ));
}
