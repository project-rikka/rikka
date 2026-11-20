use std::collections::BTreeMap;
use std::path::Path;

use crate::support::{applied, clip, film, imported, inserted, keyed, redrawn, square};
use cut_project::{
    AnimationPreset, AnimationPresetKind, AnimationPresets, Art, ArtField, Bend, Camera,
    CameraField, Direction, Document, EXTENSION, Edit, EditPoint, EffectSlot, Error, Handle,
    Height, Incoming, Interpolation, Item, Key, KeyChange, KeyRef, KeysChange, Letters, LoopKind,
    LoopPreset, Order, Outgoing, Placement, PlacementField, Preset, Project, Property, Region,
    Scalar, Side, Track, Transition, TransitionAlignment, TransitionKind, TransitionKnob, Value,
    save,
};
use rikka_effects::Kind;
use rikka_figure::{Lettering, Shape};
use rikka_keyframe::{Ease, Easing, Family};
use rikka_types::{Color, Point};

fn open_project(path: &Path) -> Project {
    (**Document::open(path).expect("opening").project()).clone()
}

fn everything() -> Project {
    let (project, clips) = imported(vec![film("beach", 4), film("dunes", 4)]);
    let footage = clips[0];
    let mut blur = Kind::GaussianBlur.default_effect();
    blur.set("radius", 3.5);
    let bounce = Preset::whole(Easing::new(Family::Bounce, Ease::Out));
    let project = applied(
        &project,
        Edit::Effects {
            clip: footage,
            effects: vec![
                EffectSlot {
                    tracks: BTreeMap::from([(
                        "radius".to_owned(),
                        Track::new([
                            Key::<Scalar>::new(0, 1.0).with_outgoing(Outgoing::Preset(bounce)),
                            Key::new(30, 4.0)
                                .with_incoming(Incoming::Bezier(Handle::new(0.5, 0.2)))
                                .with_outgoing(Outgoing::Bezier(Handle::new(-0.25, 0.7))),
                            Key::new(60, 9.0).with_outgoing(Outgoing::Hold),
                            Key::new(80, 2.0),
                        ]),
                    )]),
                    ..EffectSlot::new(blur)
                },
                EffectSlot {
                    enabled: false,
                    ..EffectSlot::new(Kind::Vibrance.default_effect())
                },
            ],
        },
    );
    let project = keyed(
        &project,
        footage,
        Property::Placement(PlacementField::Opacity),
        KeyChange::Add { at: 0 },
    );
    let solid = Placement {
        depth: 120.0,
        anchor: Point::new(5.0, -5.0),
        aspect: -0.25,
        tilt_x: 10.0,
        tilt_y: -20.0,
        three_d: true,
        ..Placement::default()
    };
    let (project, shape) = inserted(&project, square(50.0).with_placement(solid), 5, 40);
    let rotation = Property::Placement(PlacementField::Rotation);
    let project = keyed(&project, shape, rotation.clone(), KeyChange::Add { at: 5 });
    let visual = project.visual(clip(&project, shape), 30).expect("a visual");
    let project = redrawn(
        &project,
        shape,
        visual.moved(Point::new(15.0, 25.0)).appearance(),
        30,
    );
    let position = Property::Placement(PlacementField::Position);
    let project = keyed(&project, shape, position.clone(), KeyChange::Add { at: 5 });
    let project = keyed(&project, shape, position.clone(), KeyChange::Add { at: 25 });
    let project = keyed(
        &project,
        shape,
        position.clone(),
        KeyChange::Value {
            at: 25,
            value: Value::Point(Point::new(-90.0, 40.0)),
        },
    );
    let project = keyed(&project, shape, position.clone(), KeyChange::Add { at: 44 });
    let project = keyed(
        &project,
        shape,
        position.clone(),
        KeyChange::tangent(44, Bend::default(), Side::Incoming, Point::new(10.0, 60.0)),
    );
    let fill = Property::Art(ArtField::Fill);
    let project = keyed(&project, shape, fill.clone(), KeyChange::Add { at: 10 });
    let project = keyed(
        &project,
        shape,
        fill,
        KeyChange::Value {
            at: 10,
            value: Value::Color(Color::rgba(0.25, 0.5, 0.75, 0.5)),
        },
    );
    let project = applied(
        &project,
        Edit::Keys {
            clip: shape,
            keys: vec![KeyRef::new(position, 25), KeyRef::new(rotation, 5)],
            change: KeysChange::Interpolate(Interpolation::Auto),
        },
    );
    let project = applied(
        &project,
        Edit::AnimationPresets {
            clip: shape,
            animation_presets: AnimationPresets {
                in_preset: Some(AnimationPreset {
                    letters: Some(Letters::new(Order::Random(3))),
                    ..AnimationPreset::new_in(AnimationPresetKind::Slide)
                }),
                out_preset: Some(AnimationPreset::new_out(AnimationPresetKind::Wipe)),
                loop_preset: Some(LoopPreset {
                    phase: 4,
                    ..LoopPreset::new(LoopKind::Shake)
                }),
            },
        },
    );
    let (project, banner) = inserted(
        &project,
        Region {
            feather: 8.0,
            inverted: true,
            ..Region::shaped(
                Shape::Star {
                    points: 6,
                    depth: 0.4,
                },
                300.0,
                120.0,
            )
        },
        0,
        80,
    );
    let project = applied(
        &project,
        Edit::Effects {
            clip: banner,
            effects: vec![EffectSlot::new(Kind::Mosaic.default_effect())],
        },
    );
    let (project, _) = inserted(&project, Region::whole(), 20, 10);
    let (project, camera) = inserted(
        &project,
        Item::Camera(Camera {
            dolly: 200.0,
            angle: 50.0,
            ..Camera::default()
        }),
        0,
        60,
    );
    let interest = Property::Camera(CameraField::Interest);
    let project = keyed(&project, camera, interest.clone(), KeyChange::Add { at: 0 });
    let project = keyed(
        &project,
        camera,
        interest.clone(),
        KeyChange::Add { at: 50 },
    );
    let project = keyed(
        &project,
        camera,
        interest,
        KeyChange::Value {
            at: 50,
            value: Value::Point(Point::new(300.0, 0.0)),
        },
    );
    let text = square(10.0).with_placement(Placement::default());
    let (project, caption) = inserted(
        &project,
        cut_project::Graphic {
            art: Art::Text {
                lettering: Lettering {
                    text: "Hello".into(),
                    size: 48.0,
                    ..Lettering::default()
                },
                reveal: 0.25,
            },
            ..text
        },
        0,
        30,
    );
    let project = keyed(
        &project,
        caption,
        Property::Art(ArtField::Reveal),
        KeyChange::Add { at: 12 },
    );
    let push = Transition {
        direction: Direction::Up,
        sound: false,
        ..Transition::new(TransitionKind::Push, project.rate())
    };
    let push = Transition {
        tuning: push
            .tuning
            .with(TransitionKind::Push, TransitionKnob::Shutter, 140.0),
        ..push.with_alignment(TransitionAlignment::Start)
    };
    let flash = Transition::new(TransitionKind::Flash, project.rate());
    let flash = Transition {
        tuning: flash.tuning.tinted(Color::rgb(1.0, 0.2, 0.4)),
        ..flash
    };
    let project = applied(
        &project,
        Edit::Transitions {
            edit_points: vec![EditPoint::tail(footage)],
            transition: Some(push),
        },
    );
    let project = applied(
        &project,
        Edit::Transitions {
            edit_points: vec![EditPoint::tail(clips[1])],
            transition: Some(flash),
        },
    );
    let project = applied(&project, Edit::ResizeAll(Height::LARGE));
    applied(
        &project,
        Edit::Resize {
            layer: 1,
            height: Height::new(150.0),
        },
    )
}

#[test]
fn animation_round_trips_through_file() {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join(format!("animated.{EXTENSION}"));
    let project = everything();

    save(&project, &path).expect("saving");

    assert_eq!(open_project(&path), project);
    let text = std::fs::read_to_string(&path).expect("reading");
    assert!(text.contains("\"format\": \"rikka-cut\""));
    assert!(text.contains("\"version\": 1"));
    assert!(text.contains("\"kind\": \"gaussian_blur\""));
    assert!(text.contains("\"position\""));
    assert!(text.contains("\"camera\""));
    assert!(text.contains("\"kind\": \"push\""));
    assert!(text.contains("\"kind\": \"flash\""));
}

#[test]
fn media_moved_with_the_project_stays_linked() {
    let root = tempfile::tempdir().expect("a folder");
    let before = root.path().join("before");
    let files = ["work/media/beach.mp4", "shared/dunes.mp4"];
    for file in files {
        let path = before.join(file);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("folders");
        std::fs::write(&path, b"").expect("a file");
    }
    let media = files
        .iter()
        .map(|file| {
            let stem = Path::new(file).file_stem().expect("a stem");
            let original = film(&stem.to_string_lossy(), 4);
            cut_project::Media::new(before.join(file), original.duration())
                .with_picture(original.picture().expect("a picture"))
        })
        .collect();
    let (project, clips) = imported(media);
    save(&project, &before.join("work/edit.rcut")).expect("saving");

    let after = root.path().join("after");
    std::fs::rename(&before, &after).expect("moving");
    let reopened = open_project(&after.join("work/edit.rcut"));

    for (id, file) in clips.into_iter().zip(files) {
        let media = clip(&reopened, id).media().expect("footage");
        let path = reopened.media(media).expect("media").path();
        let moved = std::fs::canonicalize(after.join(file)).expect("moved");
        assert_eq!(path, moved, "{file}");
    }
}

#[test]
fn invalid_keys_tracks_and_effects_are_rejected() {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join("broken.rcut");
    save(&everything(), &path).expect("saving");
    let text = std::fs::read_to_string(&path).expect("reading");
    let refused = |text: String| {
        std::fs::write(&path, text).expect("writing");
        matches!(
            Document::open(&path),
            Err(Error::Damaged { .. } | Error::File(_))
        )
    };

    assert!(refused(text.replacen("\"at\": 60", "\"at\": 600", 1)));
    assert!(refused(text.replacen("\"at\": 60", "\"at\": -1", 1)));
    assert!(refused(text.replacen("gaussian_blur", "teleport", 1)));
    assert!(refused(text.replacen("\"position\"", "\"teleport\"", 1)));
    assert!(refused(text.replacen("\"from\": 0.0", "\"from\": 2.0", 1)));
    assert!(refused(text.replacen(
        "\"kind\": \"hold\"",
        "\"kind\": \"teleport\"",
        1
    )));
    assert!(refused(text.replacen("\"before\": 0", "\"before\": -3", 1)));
    assert!(refused(text.replacen("\"push\"", "\"teleport\"", 1)));
}

#[test]
fn files_beyond_the_editors_limits_are_rejected() {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join("hostile.rcut");
    let (project, _) = imported(vec![film("a", 4)]);
    save(&project, &path).expect("saving");
    let text = std::fs::read_to_string(&path).expect("reading");
    let far = (i64::MAX - 1).to_string();
    for (from, to) in [
        ("\"width\": 1280", "\"width\": 4000000000".to_owned()),
        ("\"height\": 720", "\"height\": 8".to_owned()),
        ("\"start\": 0", format!("\"start\": {far}")),
        ("\"source_in\": 0", format!("\"source_in\": {far}")),
        ("\"mark_in\": null", format!("\"mark_in\": {far}")),
    ] {
        assert!(text.contains(from), "{from}");
        std::fs::write(&path, text.replacen(from, &to, 1)).expect("writing");
        assert!(
            matches!(Document::open(&path), Err(Error::Damaged { .. })),
            "{to}"
        );
    }
}

#[test]
fn edits_beyond_the_longest_project_are_refused() {
    let (project, clips) = imported(vec![film("a", 4)]);
    let moved = project.apply(&Edit::Move {
        clips: vec![clips[0]],
        by: Project::LONGEST,
        across: 0,
    });
    assert!(matches!(moved, Err(Error::OutOfReach)));
}
