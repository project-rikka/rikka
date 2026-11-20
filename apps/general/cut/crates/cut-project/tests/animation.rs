use crate::support::{
    applied, clip, clips_on, close, inserted, keyed, near, near_point, redrawn, square,
};
use cut_project::{
    Action, Appearance, ArtField, Camera, Channel, Clip, ClipId, Document, Edge, Edit, EffectSlot,
    Handle, Incoming, Interpolation, Item, Key, KeyChange, KeyRef, KeysChange, Outgoing,
    PlacementField, Project, Property, Region, Scalar, Smooth, Track, Value,
};
use rikka_effects::Kind;
use rikka_types::Point;
use std::collections::BTreeMap;

const POSITION: Property = Property::Placement(PlacementField::Position);

fn x_at(project: &Project, id: ClipId, frame: i64) -> f32 {
    let subject = clip(project, id);
    project
        .visual(subject, frame)
        .expect("a visual")
        .placement()
        .x
}

fn pulled(project: &Project, id: ClipId, frame: i64, by: Point) -> Project {
    let visual = project.visual(clip(project, id), frame).expect("a visual");
    redrawn(project, id, visual.moved(by).appearance(), frame)
}

fn sliding() -> (Project, ClipId) {
    let (project, id) = inserted(&Project::new(), square(100.0), 0, 50);
    let project = keyed(&project, id, POSITION, KeyChange::Add { at: 0 });
    let project = pulled(&project, id, 40, Point::new(400.0, 0.0));
    (project, id)
}

fn positions(project: &Project, id: ClipId) -> Vec<(i64, Point)> {
    match clip(project, id).channel(&POSITION) {
        Some(Channel::Spatial(track)) => {
            track.keys().iter().map(|key| (key.at, key.value)).collect()
        }
        _ => Vec::new(),
    }
}

#[test]
fn keyed_redraw_adds_key() {
    let (project, id) = sliding();

    assert_eq!(
        positions(&project, id),
        [(0, Point::ZERO), (40, Point::new(400.0, 0.0))]
    );
    assert_eq!(clip(&project, id).placement().x, 0.0);
    assert!(near(x_at(&project, id, 20), 200.0, 1e-3));
    assert_eq!(x_at(&project, id, 45), 400.0);
    let positions: Vec<f32> = project
        .visuals_at(10)
        .map(|(_, _, visual)| visual.placement().x)
        .collect();
    assert!(near(positions[0], 100.0, 1e-3));
}

#[test]
fn keyed_rotation_keeps_direction() {
    let (project, id) = inserted(&Project::new(), square(100.0), 0, 50);
    let rotation = Property::Placement(PlacementField::Rotation);
    let project = keyed(&project, id, rotation.clone(), KeyChange::Add { at: 0 });
    let turn = |project: &Project, frame: i64, degrees: f32| {
        let visual = project.visual(clip(project, id), frame).expect("a visual");
        let mut graphic = visual.graphic().expect("a graphic").clone();
        graphic.placement.rotation = (graphic.placement.rotation + degrees).rem_euclid(360.0);
        redrawn(project, id, Appearance::Graphic(graphic.checked()), frame)
    };

    let project = turn(&project, 10, 170.0);
    let project = turn(&project, 10, 30.0);

    let channel = clip(&project, id).channel(&rotation).expect("a track");
    assert_eq!(channel.value(10), Some(Value::Scalar(200.0)));
    let halfway = project.visual(clip(&project, id), 5).expect("a visual");
    assert!(close(halfway.placement().rotation, 100.0));
}

#[test]
fn key_changes_undo_as_one_step() {
    let (project, id) = sliding();
    let mut document = Document::new();
    document
        .apply(&Edit::Paste {
            pieces: project.pieces(&[id]),
            at: 0,
            layer: 0,
        })
        .expect("pasting");
    let pasted = document.project().layers()[0].clips()[0].id();

    document
        .apply(&Edit::Key {
            clip: pasted,
            property: POSITION,
            change: KeyChange::Add { at: 20 },
        })
        .expect("keying");
    assert_eq!(document.undoable(), Some(Action::Key));
    assert_eq!(positions(document.project(), pasted).len(), 3);

    document.undo();
    assert_eq!(positions(document.project(), pasted).len(), 2);
}

#[test]
fn keys_move_with_clip() {
    let (project, id) = sliding();

    let moved = applied(
        &project,
        Edit::Move {
            clips: vec![id],
            by: 100,
            across: 1,
        },
    );

    for frame in 0..50 {
        assert_eq!(x_at(&moved, id, frame + 100), x_at(&project, id, frame));
    }
}

#[test]
fn split_divides_track_without_jump() {
    let (project, id) = sliding();

    let split = applied(
        &project,
        Edit::Split {
            clips: vec![id],
            at: 25,
        },
    );

    let halves = clips_on(&split, 0);
    let (left, right) = (halves[0].id(), halves[1].id());
    assert_eq!(positions(&split, left).len(), 2);
    assert!(near(positions(&split, left)[1].1.x, 240.0, 1e-3));
    assert!(near(positions(&split, right)[0].1.x, 250.0, 1e-3));
    for frame in 0..50 {
        let half = halves
            .iter()
            .find(|half| half.covers(frame))
            .expect("a half");
        assert!(near(
            x_at(&split, half.id(), frame),
            x_at(&project, id, frame),
            1e-3
        ));
    }
}

const RADIUS: &str = "radius";

fn knob() -> Property {
    Property::Effect {
        slot: 0,
        knob: RADIUS.to_owned(),
    }
}

fn blurred(track: Track<Scalar>) -> (Project, ClipId) {
    let (project, id) = inserted(&Project::new(), square(100.0), 0, 90);
    let project = applied(
        &project,
        Edit::Effects {
            clip: id,
            effects: vec![EffectSlot {
                tracks: BTreeMap::from([(RADIUS.to_owned(), track)]),
                ..EffectSlot::new(Kind::GaussianBlur.default_effect())
            }],
        },
    );
    (project, id)
}

fn sampled(clip: &Clip, property: &Property, frame: f64) -> Value {
    let at = frame - clip.start() as f64;
    match clip.channel(property).expect("an animated property") {
        Channel::Scalar(track) => Value::Scalar(track.sample(at).expect("a value")),
        Channel::Spatial(track) => Value::Point(track.sample(at).expect("a value")),
        Channel::Planar(track) => Value::Point(track.sample(at).expect("a value")),
        Channel::Chroma(track) => Value::Color(track.sample(at).expect("a value")),
    }
}

fn same(left: Value, right: Value) -> bool {
    match (left, right) {
        (Value::Scalar(left), Value::Scalar(right)) => near(left, right, 1e-3),
        (Value::Point(left), Value::Point(right)) => near_point(left, right, 2e-3),
        _ => false,
    }
}

fn split_keeps(project: &Project, id: ClipId, property: &Property, at: i64) {
    let whole = clip(project, id).clone();
    let split = applied(
        project,
        Edit::Split {
            clips: vec![id],
            at,
        },
    );
    let halves = clips_on(&split, 0);
    for step in 0..(whole.length() * 4) {
        let frame = whole.start() as f64 + f64::from(step as i32) / 4.0;
        let half = halves
            .iter()
            .find(|half| half.covers(frame.floor() as i64))
            .expect("a half");
        if frame > (half.end() - 1) as f64 {
            continue;
        }
        let (before, after) = (
            sampled(&whole, property, frame),
            sampled(half, property, frame),
        );
        assert!(
            same(before, after),
            "at {frame}: {before:?} became {after:?}"
        );
    }
}

#[test]
fn split_bezier_segment_keeps_curve() {
    let track = Track::new([
        Key::new(0, 2.0).with_outgoing(Outgoing::Bezier(Handle::new(3.0, 0.6))),
        Key::new(50, 40.0).with_incoming(Incoming::Bezier(Handle::new(-1.0, 0.2))),
        Key::new(80, 10.0),
    ]);
    let (project, id) = blurred(track);

    for at in [7, 25, 49, 51, 66] {
        split_keeps(&project, id, &knob(), at);
    }
}

#[test]
fn trim_crops_eased_motion_path() {
    let (project, id) = inserted(&Project::new(), square(100.0), 0, 60);
    let project = keyed(&project, id, POSITION, KeyChange::Add { at: 0 });
    let project = keyed(&project, id, POSITION, KeyChange::Add { at: 30 });
    let project = keyed(
        &project,
        id,
        POSITION,
        KeyChange::Value {
            at: 30,
            value: Value::Point(Point::new(300.0, -150.0)),
        },
    );
    let project = keyed(&project, id, POSITION, KeyChange::Add { at: 59 });
    let project = keyed(
        &project,
        id,
        POSITION,
        KeyChange::Value {
            at: 59,
            value: Value::Point(Point::new(600.0, 100.0)),
        },
    );
    let project = applied(
        &project,
        Edit::Keys {
            clip: id,
            keys: vec![KeyRef::new(POSITION, 30)],
            change: KeysChange::Interpolate(Interpolation::EasyEase),
        },
    );
    let bend = clip(&project, id)
        .channel(&POSITION)
        .and_then(|channel| channel.bend(30))
        .expect("a bend");
    assert_eq!(bend.smooth, Smooth::Auto);
    assert!(bend.outgoing.y != 0.0 || bend.outgoing.x != 0.0);

    for at in [10, 30, 44] {
        split_keeps(&project, id, &POSITION, at);
    }
    let trimmed = applied(
        &project,
        Edit::Trim {
            clips: vec![id],
            edge: Edge::Head,
            by: 12,
        },
    );
    let subject = clip(&trimmed, id);
    for step in 12 * 4..60 * 4 - 3 {
        let frame = f64::from(step) / 4.0;
        assert!(same(
            sampled(clip(&project, id), &POSITION, frame),
            sampled(subject, &POSITION, frame)
        ));
    }
    let frozen = subject
        .channel(&POSITION)
        .and_then(|channel| channel.bend(30 - 12))
        .expect("a bend");
    assert_eq!(frozen.smooth, Smooth::Continuous);
}

const ZOOM: Property = Property::Placement(PlacementField::Zoom);
const OPACITY: Property = Property::Placement(PlacementField::Opacity);

fn frames(project: &Project, id: ClipId, property: &Property) -> Vec<i64> {
    clip(project, id)
        .channel(property)
        .map(|channel| channel.frames())
        .unwrap_or_default()
}

fn valued(project: &Project, id: ClipId, property: Property, at: i64, value: f32) -> Project {
    let project = keyed(project, id, property.clone(), KeyChange::Add { at });
    keyed(
        &project,
        id,
        property,
        KeyChange::Value {
            at,
            value: Value::Scalar(value),
        },
    )
}

fn zooming() -> (Project, ClipId) {
    let (project, id) = inserted(&Project::new(), square(100.0), 10, 50);
    let project = valued(&project, id, ZOOM, 10, 1.0);
    let project = valued(&project, id, ZOOM, 20, 2.0);
    let project = valued(&project, id, ZOOM, 40, 3.0);
    let project = valued(&project, id, OPACITY, 20, 0.5);
    (project, id)
}

fn rekeyed(project: &Project, id: ClipId, keys: &[(Property, i64)], change: KeysChange) -> Project {
    applied(
        project,
        Edit::Keys {
            clip: id,
            keys: keys
                .iter()
                .map(|(property, at)| KeyRef::new(property.clone(), *at))
                .collect(),
            change,
        },
    )
}

#[test]
fn moved_keys_overwrite_targets() {
    let (project, id) = zooming();

    let moved = rekeyed(
        &project,
        id,
        &[(ZOOM, 20), (OPACITY, 20)],
        KeysChange::Move { by: 20 },
    );

    assert_eq!(frames(&moved, id, &ZOOM), [0, 30]);
    assert_eq!(
        clip(&moved, id)
            .channel(&ZOOM)
            .and_then(|channel| channel.value(30)),
        Some(Value::Scalar(2.0))
    );
    assert_eq!(frames(&moved, id, &OPACITY), [30]);
}

#[test]
fn pasted_keys_skip_missing_properties() {
    let (project, id) = zooming();
    let width = Property::Art(ArtField::Width);
    let project = keyed(&project, id, width.clone(), KeyChange::Add { at: 30 });
    let project = keyed(&project, id, POSITION, KeyChange::Add { at: 30 });
    let project = keyed(
        &project,
        id,
        POSITION,
        KeyChange::Value {
            at: 30,
            value: Value::Point(Point::new(5.0, 6.0)),
        },
    );
    let (project, range) = inserted(&project, Region::whole(), 100, 20);
    let piece = project.copy_keys(
        id,
        &[
            KeyRef::new(ZOOM, 20),
            KeyRef::new(ZOOM, 40),
            KeyRef::new(width, 30),
            KeyRef::new(POSITION, 30),
        ],
    );
    assert_eq!(piece.len(), 4);

    let pasted = applied(
        &project,
        Edit::PasteKeys {
            clip: range,
            keys: piece,
            at: 105,
        },
    );

    assert_eq!(frames(&pasted, range, &ZOOM), [5]);
    assert_eq!(frames(&pasted, range, &POSITION), [15]);
    assert!(
        clip(&pasted, range)
            .channel(&Property::Art(ArtField::Width))
            .is_none()
    );
    let (project, camera) = inserted(&pasted, Item::Camera(Camera::default()), 200, 30);
    let piece = project.copy_keys(id, &[KeyRef::new(ZOOM, 20)]);
    let onto_camera = applied(
        &project,
        Edit::PasteKeys {
            clip: camera,
            keys: piece,
            at: 200,
        },
    );
    assert!(!clip(&onto_camera, camera).is_animated());
}
