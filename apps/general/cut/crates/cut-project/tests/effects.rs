use std::collections::BTreeMap;

use crate::support::{applied, clip, clips_on, inserted, square};
use cut_project::{ClipId, Edit, EffectSlot, Key, Project, Track};
use rikka_effects::{Effect, Kind};

fn blur(radius: f32) -> Effect {
    let mut effect = Kind::GaussianBlur.default_effect();
    assert!(effect.set("radius", radius));
    effect
}

fn radius(effect: &Effect) -> f32 {
    effect.value("radius").expect("a radius")
}

fn growing_blur() -> EffectSlot {
    EffectSlot {
        tracks: BTreeMap::from([(
            "radius".to_owned(),
            Track::new([Key::new(0, 2.0), Key::new(10, 12.0)]),
        )]),
        ..EffectSlot::new(blur(1.0))
    }
}

fn stacked(project: &Project, id: ClipId, effects: Vec<EffectSlot>) -> Project {
    applied(project, Edit::Effects { clip: id, effects })
}

fn square_at_ten() -> (Project, ClipId) {
    inserted(&Project::new(), square(100.0), 10, 40)
}

#[test]
fn sampled_effects_are_enabled_and_keyed() {
    let (project, id) = square_at_ten();
    let off = EffectSlot {
        enabled: false,
        ..EffectSlot::new(Kind::Mosaic.default_effect())
    };
    let project = stacked(&project, id, vec![growing_blur(), off]);

    let subject = clip(&project, id);
    let effects = subject.effects_at(15);
    assert_eq!(effects.len(), 1);
    assert_eq!(radius(&effects[0]), 7.0);
    assert_eq!(radius(&subject.effects_at(10)[0]), 2.0);
    assert_eq!(radius(&subject.effects_at(30)[0]), 12.0);
    assert!(subject.is_animated());
}

#[test]
fn effects_follow_splits_and_copies() {
    let (project, id) = square_at_ten();
    let project = stacked(&project, id, vec![growing_blur()]);

    let split = applied(
        &project,
        Edit::Split {
            clips: vec![id],
            at: 15,
        },
    );
    let halves = clips_on(&split, 0);
    assert_eq!(radius(&halves[0].effects_at(14)[0]), 6.0);
    assert_eq!(radius(&halves[1].effects_at(15)[0]), 7.0);
    assert_eq!(radius(&halves[1].effects_at(20)[0]), 12.0);

    let (pasted, applied) = project
        .apply(&Edit::Paste {
            pieces: project.pieces(&[id]),
            at: 100,
            layer: 0,
        })
        .expect("pasting");
    let copy = applied.selected.expect("the copy")[0];
    assert_eq!(clip(&pasted, copy).effects(), clip(&project, id).effects());
}
