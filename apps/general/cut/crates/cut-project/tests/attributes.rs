use std::collections::BTreeMap;

use crate::support::{applied, clip, film, imported, inserted, song, square};
use cut_project::{
    Appearance, Attribute, ClipId, Edit, EffectSlot, Error, Key, KeyFit, Piece, Project, Redraw,
    Switch, Track, TweenChange,
};
use rikka_effects::Kind;

fn growing_blur() -> EffectSlot {
    EffectSlot {
        tracks: BTreeMap::from([(
            "radius".to_owned(),
            Track::new([Key::new(0, 2.0), Key::new(10, 12.0)]),
        )]),
        ..EffectSlot::new(Kind::GaussianBlur.default_effect())
    }
}

fn copied(project: &Project, id: ClipId) -> Box<Piece> {
    let [piece] = &project.pieces(&[id])[..] else {
        panic!("one piece");
    };
    Box::new(piece.clone())
}

fn pasted(
    project: &Project,
    from: ClipId,
    onto: ClipId,
    attributes: Vec<Attribute>,
    keys: KeyFit,
) -> Result<Project, Error> {
    project
        .apply(&Edit::PasteAttributes {
            from: copied(project, from),
            onto: vec![onto],
            attributes,
            keys,
        })
        .map(|(project, _)| project)
}

fn two_squares() -> (Project, ClipId, ClipId) {
    let (project, source) = inserted(&Project::new(), square(10.0), 0, 41);
    let project = applied(
        &project,
        Edit::Effects {
            clip: source,
            effects: vec![growing_blur()],
        },
    );
    let (project, target) = inserted(&project, square(20.0), 100, 81);
    let project = applied(
        &project,
        Edit::Effects {
            clip: target,
            effects: vec![EffectSlot::new(Kind::Mosaic.default_effect())],
        },
    );
    (project, source, target)
}

#[test]
fn pasted_effects_append_with_fitted_keys() {
    let cases = [
        (KeyFit::Maintain, vec![0, 10]),
        (KeyFit::Stretch, vec![0, 20]),
    ];
    for (keys, frames) in cases {
        let (project, source, target) = two_squares();
        let project =
            pasted(&project, source, target, vec![Attribute::Effect(0)], keys).expect("pasting");

        let effects = clip(&project, target).effects();
        let kinds: Vec<Kind> = effects.iter().map(|slot| slot.effect.kind()).collect();
        assert_eq!(kinds, [Kind::Mosaic, Kind::GaussianBlur], "{keys:?}");
        let keyed: Vec<i64> = effects[1]
            .track("radius")
            .expect("a radius track")
            .keys()
            .iter()
            .map(|key| key.at)
            .collect();
        assert_eq!(keyed, frames, "{keys:?}");
        assert_eq!(clip(&project, source).effects(), [growing_blur()]);
    }
}

#[test]
fn pastable_attributes_follow_what_targets_take() {
    let (project, clips) = imported(vec![film("beach", 4), song("tune", 4)]);
    let (beach, tune) = (clips[0], clips[1]);
    let project = applied(
        &project,
        Edit::Effects {
            clip: beach,
            effects: vec![growing_blur()],
        },
    );
    let (project, shape) = inserted(&project, square(10.0), 0, 20);
    let piece = copied(&project, beach);

    assert_eq!(
        piece.attributes(),
        [
            Attribute::Placement,
            Attribute::Picture,
            Attribute::Speed,
            Attribute::AnimationPresets,
            Attribute::Effect(0),
            Attribute::Sound,
        ]
    );
    let cases = [
        (
            shape,
            vec![
                Attribute::Placement,
                Attribute::AnimationPresets,
                Attribute::Effect(0),
            ],
        ),
        (tune, vec![Attribute::Sound]),
    ];
    for (target, expected) in cases {
        assert_eq!(project.pastable(&piece, &[target]), expected);
    }
}

#[test]
fn unfit_and_locked_pastes_are_refused() {
    let (project, clips) = imported(vec![song("tune", 4)]);
    let (project, shape) = inserted(&project, square(10.0), 0, 20);
    let refused = pasted(
        &project,
        clips[0],
        shape,
        vec![Attribute::Sound],
        KeyFit::Maintain,
    );
    assert!(matches!(refused, Err(Error::NothingToPaste)));

    let (project, source, target) = two_squares();
    let (layer, _) = project.locate(target).expect("there");
    let locked = applied(
        &project,
        Edit::Set {
            layer,
            switch: Switch::Locked,
            on: true,
        },
    );
    let refused = pasted(
        &locked,
        source,
        target,
        vec![Attribute::Effect(0)],
        KeyFit::Maintain,
    );
    assert!(matches!(refused, Err(Error::Locked)));
}

#[test]
fn pasted_tween_spans_the_target() {
    let (project, source, target) = two_squares();
    let project = applied(
        &project,
        Edit::Tween {
            clip: source,
            change: TweenChange::On,
        },
    );
    let project = pasted(
        &project,
        source,
        target,
        vec![Attribute::Placement],
        KeyFit::Maintain,
    )
    .expect("pasting");

    let tween = clip(&project, target).tween().expect("a tween");
    let target = clip(&project, target);
    assert_eq!((tween.start, tween.end), (target.start(), target.end() - 1));
    assert_eq!(target.placement(), clip(&project, source).placement());
}

#[test]
fn pasted_speed_retimes_footage() {
    let (project, clips) = imported(vec![film("beach", 4), film("dunes", 8)]);
    let (beach, dunes) = (clips[0], clips[1]);
    let mut treatment = *clip(&project, beach).treatment().expect("footage");
    treatment.speed = 2.0;
    let project = applied(
        &project,
        Edit::Redraw {
            changes: vec![(beach, Appearance::Footage(treatment))],
            by: Redraw::Style,
            at: 0,
        },
    );
    let length = clip(&project, dunes).length();

    let project = pasted(
        &project,
        beach,
        dunes,
        vec![Attribute::Speed],
        KeyFit::Maintain,
    )
    .expect("pasting");

    let dunes = clip(&project, dunes);
    assert_eq!(dunes.speed(), 2.0);
    assert_eq!(dunes.length(), length / 2);
}

#[test]
fn removing_effects_clears_every_stack() {
    let (project, source, target) = two_squares();
    let project = applied(&project, Edit::RemoveEffects(vec![source, target]));
    assert!(clip(&project, source).effects().is_empty());
    assert!(clip(&project, target).effects().is_empty());

    let refused = project.apply(&Edit::RemoveEffects(vec![source]));
    assert!(matches!(refused, Err(Error::NoEffects)));
}
