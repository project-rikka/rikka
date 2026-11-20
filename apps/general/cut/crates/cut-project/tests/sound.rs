use cut_project::{
    Appearance, ClipId, Document, EXTENSION, Edge, Edit, Error, Fade, FadeShape, KeyChange,
    MixField, Project, Property, Treatment, Value, save,
};

use crate::support::{
    applied, clip, close, film, imported, inserted, keyed, redrawn, song, square,
};

const VOLUME: Property = Property::Mix(MixField::Volume);

fn treated(project: &Project, id: ClipId, change: impl FnOnce(&mut Treatment)) -> Project {
    let mut treatment = clip(project, id).treatment_at(0).expect("footage");
    change(&mut treatment);
    redrawn(project, id, Appearance::Footage(treatment), 0)
}

fn gain(project: &Project, id: ClipId, at: f64) -> f32 {
    clip(project, id).level_at(at, None).expect("sound").gain
}

#[test]
fn volume_keys_shape_level() {
    let (project, ids) = imported(vec![film("talk", 4)]);
    let project = keyed(&project, ids[0], VOLUME, KeyChange::Add { at: 0 });
    let project = keyed(&project, ids[0], VOLUME, KeyChange::Add { at: 20 });
    let project = keyed(
        &project,
        ids[0],
        VOLUME,
        KeyChange::Value {
            at: 20,
            value: Value::Scalar(0.0),
        },
    );

    assert!(close(gain(&project, ids[0], 0.0), 1.0));
    assert!(close(gain(&project, ids[0], 10.0), 0.5));
    assert!(close(gain(&project, ids[0], 15.5), 0.225));
    assert!(close(gain(&project, ids[0], 60.0), 0.0));
}

#[test]
fn volume_change_on_key_updates_key() {
    let (project, ids) = imported(vec![film("talk", 4)]);
    let project = keyed(&project, ids[0], VOLUME, KeyChange::Add { at: 0 });
    let project = keyed(&project, ids[0], VOLUME, KeyChange::Add { at: 40 });

    let mut treatment = clip(&project, ids[0]).treatment_at(40).expect("footage");
    treatment.mix.volume = 0.25;
    let project = redrawn(&project, ids[0], Appearance::Footage(treatment), 40);

    let subject = clip(&project, ids[0]);
    assert!(close(subject.treatment().expect("footage").mix.volume, 1.0));
    assert!(close(subject.mix_at(40).expect("sound").volume, 0.25));
}

#[test]
fn fades_shape_level_and_follow_trims() {
    let (project, ids) = imported(vec![film("talk", 4)]);
    let project = treated(&project, ids[0], |treatment| {
        treatment.mix.fade_in = Fade {
            length: 10,
            shape: FadeShape::Linear,
        };
        treatment.mix.fade_out = Fade {
            length: 20,
            shape: FadeShape::EqualPower,
        };
    });

    assert!(close(gain(&project, ids[0], 5.0), 0.5));
    assert!(close(gain(&project, ids[0], 50.0), 1.0));
    assert!(close(
        gain(&project, ids[0], 90.0),
        (0.25 * std::f32::consts::PI).sin()
    ));

    let trimmed = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[0]],
            edge: Edge::Head,
            by: 30,
        },
    );
    assert!(close(gain(&trimmed, ids[0], 35.0), 0.5));

    let short = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[0]],
            edge: Edge::Tail,
            by: -88,
        },
    );
    let mix = clip(&short, ids[0]).treatment().expect("footage").mix;
    assert_eq!((mix.fade_in.length, mix.fade_out.length), (10, 2));
}

#[test]
fn volume_keys_require_audio() {
    let (project, ids) = imported(vec![song("theme", 4)]);
    let (project, shape) = inserted(&project, square(100.0), 0, 30);

    let cases = [
        (shape, VOLUME, false),
        (ids[0], VOLUME, true),
        (ids[0], Property::Mix(MixField::Pan), true),
        (
            ids[0],
            Property::Placement(cut_project::PlacementField::Opacity),
            false,
        ),
    ];
    for (id, property, accepted) in cases {
        let result = project.apply(&Edit::Key {
            clip: id,
            property: property.clone(),
            change: KeyChange::Add { at: 0 },
        });
        match accepted {
            true => assert!(result.is_ok(), "{property:?}"),
            false => assert!(matches!(result, Err(Error::NotAnimatable)), "{property:?}"),
        }
    }
}

#[test]
fn audio_keys_and_fades_round_trip() {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join(format!("sound.{EXTENSION}"));
    let (project, ids) = imported(vec![film("talk", 4)]);
    let project = keyed(&project, ids[0], VOLUME, KeyChange::Add { at: 5 });
    let project = keyed(
        &project,
        ids[0],
        Property::Mix(MixField::Pan),
        KeyChange::Add { at: 30 },
    );
    let project = treated(&project, ids[0], |treatment| {
        treatment.mix.fade_out = Fade {
            length: 15,
            shape: FadeShape::SCurve,
        };
    });

    save(&project, &path).expect("saving");
    let document = Document::open(&path).expect("opening");

    assert_eq!(**document.project(), project);
    assert!(clip(document.project(), ids[0]).is_keyed(&VOLUME));
}
