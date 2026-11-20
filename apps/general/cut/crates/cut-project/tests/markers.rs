use cut_project::{
    ClipId, Document, EXTENSION, Edge, Edit, Error, Marker, MarkerChange, MarkerColor, MarkerId,
    MarkerScope, Project, Switch, save,
};

use crate::support::{applied, clip, film, imported};

fn marked(project: &Project, on: MarkerScope, marker: Marker) -> (Project, MarkerId) {
    let (project, applied) = project
        .apply(&Edit::Marker {
            on,
            change: MarkerChange::Add(marker),
        })
        .expect("marking");
    (project, applied.marker.expect("the new marker"))
}

fn marker_spans(project: &Project, id: ClipId) -> Vec<std::ops::Range<i64>> {
    let subject = clip(project, id);
    subject
        .markers()
        .iter()
        .filter_map(|(_, marker)| subject.marker_span(marker))
        .collect()
}

#[test]
fn timeline_markers_add_set_remove() {
    let project = Project::new();
    let (project, chapter) = marked(&project, MarkerScope::Timeline, Marker::new(48));
    let (project, _) = marked(&project, MarkerScope::Timeline, Marker::new(12));

    let renamed = Marker {
        name: "Chapter".to_owned(),
        note: "Opening".to_owned(),
        color: MarkerColor::Blue,
        length: 24,
        ..Marker::new(60)
    };
    let project = applied(
        &project,
        Edit::Marker {
            on: MarkerScope::Timeline,
            change: MarkerChange::Set(chapter, renamed.clone()),
        },
    );
    let spans: Vec<(i64, i64)> = project
        .markers()
        .iter()
        .map(|(_, marker)| (marker.at, marker.end()))
        .collect();
    assert_eq!(spans, vec![(12, 12), (60, 84)]);
    assert_eq!(
        project.marker(MarkerScope::Timeline, chapter),
        Some(&renamed)
    );
    assert_eq!(project.marker_frames(), vec![12, 60]);

    let project = applied(
        &project,
        Edit::Marker {
            on: MarkerScope::Timeline,
            change: MarkerChange::Remove(chapter),
        },
    );
    assert_eq!(project.markers().len(), 1);
    assert!(matches!(
        project.apply(&Edit::Marker {
            on: MarkerScope::Timeline,
            change: MarkerChange::Remove(chapter),
        }),
        Err(Error::UnknownMarker)
    ));
    let cleared = applied(
        &project,
        Edit::Marker {
            on: MarkerScope::Timeline,
            change: MarkerChange::Clear,
        },
    );
    assert!(cleared.markers().is_empty());
}

#[test]
fn clip_markers_follow_clip_edits() {
    let (project, ids) = imported(vec![film("a", 4)]);
    let project = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[0]],
            edge: Edge::Head,
            by: 20,
        },
    );
    let (project, _) = marked(&project, MarkerScope::Clip(ids[0]), Marker::new(50));
    assert_eq!(marker_spans(&project, ids[0]), vec![50..50]);

    let moved = applied(
        &project,
        Edit::Move {
            clips: vec![ids[0]],
            by: 30,
            across: 0,
        },
    );
    assert_eq!(marker_spans(&moved, ids[0]), vec![80..80]);

    let slipped = applied(
        &project,
        Edit::Slip {
            clips: vec![ids[0]],
            by: -10,
        },
    );
    assert_eq!(marker_spans(&slipped, ids[0]), vec![60..60]);

    let split = applied(
        &project,
        Edit::Split {
            clips: vec![ids[0]],
            at: 40,
        },
    );
    let right = split.layers()[0].clips()[1].id();
    assert!(marker_spans(&split, ids[0]).is_empty());
    assert_eq!(marker_spans(&split, right), vec![50..50]);
}

#[test]
fn locked_layers_reject_clip_markers() {
    let (project, ids) = imported(vec![film("a", 4)]);
    let project = applied(
        &project,
        Edit::Set {
            layer: 0,
            switch: Switch::Locked,
            on: true,
        },
    );

    assert!(matches!(
        project.apply(&Edit::Marker {
            on: MarkerScope::Clip(ids[0]),
            change: MarkerChange::Add(Marker::new(0)),
        }),
        Err(Error::Locked)
    ));
}

#[test]
fn ripple_delete_shifts_later_markers() {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4)]);
    let (project, _) = marked(&project, MarkerScope::Timeline, Marker::new(150));

    let rippled = applied(&project, Edit::RippleDelete(vec![ids[0]]));

    let frames: Vec<i64> = rippled
        .markers()
        .iter()
        .map(|(_, marker)| marker.at)
        .collect();
    assert_eq!(frames, vec![50]);
}

#[test]
fn markers_round_trip() {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join(format!("marked.{EXTENSION}"));
    let (project, ids) = imported(vec![film("a", 4)]);
    let (project, _) = marked(
        &project,
        MarkerScope::Timeline,
        Marker {
            name: "Intro".to_owned(),
            color: MarkerColor::Red,
            length: 10,
            ..Marker::new(5)
        },
    );
    let (project, _) = marked(
        &project,
        MarkerScope::Clip(ids[0]),
        Marker {
            note: "Smile".to_owned(),
            ..Marker::new(30)
        },
    );

    save(&project, &path).expect("saving");
    let document = Document::open(&path).expect("opening");
    let reopened = document.project();

    let contents = |project: &Project| -> Vec<Marker> {
        project
            .markers()
            .iter()
            .chain(clip(project, ids[0]).markers().iter())
            .map(|(_, marker)| marker.clone())
            .collect()
    };
    assert_eq!(contents(reopened), contents(&project));
}
