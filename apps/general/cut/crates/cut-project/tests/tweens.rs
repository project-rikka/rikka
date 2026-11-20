use crate::support::{applied, clip, inserted, near, redrawn, square};
use cut_project::{
    Appearance, Document, EXTENSION, Edit, Graphic, Placement, PlacementField, Project, Property,
    Tween, TweenChange, Value, save,
};
use rikka_keyframe::{Ease, Easing, Family};

const POSITION: Property = Property::Placement(PlacementField::Position);

fn tweened() -> (Project, cut_project::ClipId) {
    let (project, id) = inserted(&Project::new(), square(10.0), 0, 61);
    (tween(&project, id, TweenChange::On), id)
}

fn tween(project: &Project, id: cut_project::ClipId, change: TweenChange) -> Project {
    applied(project, Edit::Tween { clip: id, change })
}

fn placed(
    project: &Project,
    id: cut_project::ClipId,
    at: i64,
    change: impl FnOnce(&mut Placement),
) -> Project {
    let subject = clip(project, id);
    let mut placement = subject.placement_at(at);
    change(&mut placement);
    let art = subject.art_at(at).expect("a graphic");
    redrawn(
        project,
        id,
        Appearance::Graphic(Graphic { placement, art }),
        at,
    )
}

fn x_at(project: &Project, id: cut_project::ClipId, at: i64) -> f32 {
    clip(project, id).placement_at(at).x
}

fn travelling() -> (Project, cut_project::ClipId) {
    let (project, id) = tweened();
    let project = placed(&project, id, 60, |placement| placement.x = 200.0);
    (project, id)
}

#[test]
fn new_tween_spans_whole_clip_unchanged() {
    let (project, id) = tweened();

    let subject = clip(&project, id);
    assert_eq!(
        subject.tween(),
        Some(Tween {
            start: 0,
            end: 60,
            easing: Tween::EASING,
        })
    );
    assert_eq!(
        subject.channel(&POSITION).map(|channel| channel.frames()),
        Some(vec![0, 60])
    );
    assert_eq!(subject.placement_at(30), subject.placement());
}

#[test]
fn tween_rotation_keeps_whole_turns() {
    let (project, id) = tweened();
    let project = placed(&project, id, 60, |placement| placement.rotation = 170.0);

    let project = placed(&project, id, 30, |placement| placement.rotation += 20.0);

    let rotation = clip(&project, id)
        .channel(&Property::Placement(PlacementField::Rotation))
        .expect("a turning track");
    let turned = |at: i64| rotation.value(at).and_then(Value::scalar).expect("a key");
    assert!(near(turned(0), 20.0, 1e-3));
    assert!(near(turned(60), 190.0, 1e-3));
}

#[test]
fn reversed_tween_plays_backwards() {
    let (project, id) = travelling();
    let project = tween(
        &project,
        id,
        TweenChange::Ease(Easing::new(Family::Quad, Ease::In)),
    );

    let reversed = tween(&project, id, TweenChange::Reverse);

    assert_eq!(x_at(&reversed, id, 0), 200.0);
    assert_eq!(x_at(&reversed, id, 60), 0.0);
    for at in [5, 20, 30, 47] {
        assert!(near(
            x_at(&reversed, id, at),
            x_at(&project, id, 60 - at),
            1e-2
        ));
    }
    assert_eq!(
        clip(&reversed, id).tween().map(|tween| tween.easing),
        Some(Easing::new(Family::Quad, Ease::Out))
    );
}

#[test]
fn split_tween_keeps_picture() {
    let (project, id) = travelling();

    let split = applied(
        &project,
        Edit::Split {
            clips: vec![id],
            at: 20,
        },
    );

    let layer = &split.layers()[0];
    let [left, right] = layer.clips() else {
        panic!("two halves");
    };
    assert_eq!(left.tween().map(|tween| tween.end), Some(19));
    assert_eq!(right.tween().map(|tween| tween.start), Some(20));
    for at in [0, 10, 19] {
        assert!(near(left.placement_at(at).x, x_at(&project, id, at), 1e-2));
    }
    for at in [20, 40, 60] {
        assert!(near(right.placement_at(at).x, x_at(&project, id, at), 1e-2));
    }
}

#[test]
fn tween_round_trips_through_file() {
    let (project, id) = travelling();
    let project = tween(
        &project,
        id,
        TweenChange::Ease(Easing::new(Family::Bounce, Ease::Out)),
    );
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join(format!("moving.{EXTENSION}"));

    save(&project, &path).expect("saving");

    let document = Document::open(&path).expect("opening");
    assert_eq!(
        clip(document.project(), id).tween(),
        clip(&project, id).tween()
    );
}
