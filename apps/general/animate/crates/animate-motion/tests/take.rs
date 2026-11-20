use animate_body::Side;
use animate_hold::{Hold, HoldTarget, Holder, Holds};
use animate_motion::{Incoming, MorphKey, Take};
use rikka_mmd::{VmdBoneFrame, VmdCurve, VmdInterpolation};
use rikka_model_core::glam::{Quat, Vec3};

fn morph(name: &str, frame: u32) -> MorphKey {
    MorphKey {
        morph: name.into(),
        frame,
        weight: 1.0,
        curve: VmdCurve::LINEAR,
    }
}

fn bone(name: &str, frame: u32) -> VmdBoneFrame {
    VmdBoneFrame {
        bone: name.into(),
        frame,
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        interpolation: VmdInterpolation::default(),
        physics_off: false,
    }
}

fn holding() -> Holds {
    let mut holds = Holds::default();
    holds.lay(Hold::new(
        Holder::Hand(Side::Left),
        HoldTarget {
            figure: None,
            at: Vec3::new(0.0, 1.0, 0.1),
            reach: 0.05,
        },
        20,
    ));
    holds
}

#[test]
fn take_holding_only_cloth_is_not_still() {
    let take = Take {
        holds: holding(),
        ..Take::default()
    };
    assert!(!take.is_still());
    assert!(Take::default().is_still());
}

#[test]
fn merge_keeps_incoming_holds() {
    let mut take = Take::default();
    take.merge(Take {
        holds: holding(),
        ..Take::default()
    });
    assert_eq!(take.holds, holding());
}

#[test]
fn incoming_motion_keeps_edits_made_while_it_loaded() {
    let snapshot = Take {
        bones: [bone("頭", 0)].into_iter().collect(),
        ..Take::default()
    };
    let mut edited = snapshot.clone();
    edited.morphs.insert(morph("あ", 12));
    edited.holds = holding();

    let mut incoming = Incoming::merging(Take {
        bones: [bone("首", 5)].into_iter().collect(),
        ..Take::default()
    });
    incoming.then(Incoming::merging(Take {
        morphs: [morph("い", 30)].into_iter().collect(),
        ..Take::default()
    }));
    let landed = incoming.onto(&edited);
    assert!(landed.morphs.key("あ", 12).is_some());
    assert!(landed.morphs.key("い", 30).is_some());
    assert_eq!(landed.holds, holding());
    let bones: Vec<&str> = landed.bones.keyed().map(String::as_str).collect();
    assert_eq!(bones, ["頭", "首"]);

    let mut replaced = incoming.clone();
    replaced.then(Incoming::replacing(snapshot.clone()));
    assert_eq!(replaced.onto(&edited), snapshot);
}
