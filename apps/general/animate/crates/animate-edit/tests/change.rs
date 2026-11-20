use std::sync::Arc;

use animate_edit::{Change, Channel, Clip, Key, KeyId, KeyKind, Keyed};
use animate_motion::{MorphKey, Part, PartKey, PartValues, Tag, Take};
use animate_sheet::Lane;
use rikka_mmd::{VmdBoneFrame, VmdCurve, VmdInterpolation};
use rikka_model_core::glam::{Quat, Vec3};
use rustc_hash::FxHashSet;

fn keyed(bone: &str, frame: u32, lift: f32) -> VmdBoneFrame {
    VmdBoneFrame {
        bone: bone.into(),
        frame,
        translation: Vec3::new(0.0, lift, 0.0),
        rotation: Quat::IDENTITY,
        interpolation: VmdInterpolation::default(),
        physics_off: false,
    }
}

fn motion() -> Take {
    Take {
        bones: [
            keyed("センター", 0, 0.0),
            keyed("センター", 10, 1.0),
            keyed("センター", 20, 2.0),
            keyed("首", 10, 5.0),
        ]
        .into_iter()
        .collect(),
        ..Take::default()
    }
}

fn id(bone: &str, frame: u32) -> KeyId {
    KeyId::new(Lane::Bone(bone.into()), frame)
}

type BoneHeights = Vec<(String, u32, f32)>;

fn bone_heights(motion: &Take) -> BoneHeights {
    motion
        .bones
        .all_keys()
        .map(|frame| (frame.bone.clone(), frame.frame, frame.translation.y))
        .collect()
}

fn round_trip(change: &Change) -> (BoneHeights, BoneHeights) {
    let mut edited = motion();
    change.apply(&mut edited);
    let after = bone_heights(&edited);
    change.revert(&mut edited);
    (after, bone_heights(&edited))
}

#[test]
fn laying_replaces_existing_key() {
    let change = Change::laying(&motion(), vec![Key::Bone(keyed("センター", 10, 9.0))]);
    let (after, before) = round_trip(&change);
    assert!(after.contains(&("センター".into(), 10, 9.0)));
    assert!(!after.contains(&("センター".into(), 10, 1.0)));
    assert_eq!(before, bone_heights(&motion()));
}

#[test]
fn deleting_removes_only_selected_keys() {
    let ids = FxHashSet::from_iter([id("センター", 10)]);
    let (after, before) = round_trip(&Change::deleting(&motion(), &ids));
    assert_eq!(after.len(), 3);
    assert!(after.contains(&("首".into(), 10, 5.0)));
    assert_eq!(before, bone_heights(&motion()));
}

#[test]
fn shifting_overwrites_and_clamps_at_zero() {
    let ids = FxHashSet::from_iter([id("センター", 0), id("センター", 10)]);
    let (after, before) = round_trip(&Change::shifting(&motion(), &ids, 10));
    let centre: Vec<(u32, f32)> = after
        .iter()
        .filter(|key| key.0 == "センター")
        .map(|key| (key.1, key.2))
        .collect();
    assert_eq!(centre, [(10, 0.0), (20, 1.0)]);
    assert_eq!(before, bone_heights(&motion()));

    let (after, _) = round_trip(&Change::shifting(&motion(), &ids, -50));
    assert_eq!(after, bone_heights(&motion()), "already at the start");
}

#[test]
fn bending_changes_only_curve() {
    let ids = FxHashSet::from_iter([id("首", 10)]);
    let eased = VmdCurve {
        first: (127, 0),
        second: (0, 127),
    };
    let change = Change::bending(motion().gather_keys(&ids), &Channel::OF_BONES, eased);
    let mut edited = motion();
    change.apply(&mut edited);
    let neck = edited.bones.key("首", 10).expect("the neck stays keyed");
    assert_eq!(neck.interpolation.rotation, eased);
    assert_eq!(neck.interpolation.x, eased);
    assert_eq!(neck.translation.y, 5.0);
    assert!(Change::bending(edited.gather_keys(&ids), &Channel::OF_BONES, eased).is_empty());
}

fn root(frame: u32, height: f32, tag: Tag) -> Key {
    Key::Part(PartKey {
        part: Part::Root,
        frame,
        tag,
        values: PartValues::Root {
            position: Vec3::new(0.0, height, 0.0),
            rotation: Quat::IDENTITY,
        },
        slopes: None,
    })
}

fn part(frame: u32) -> KeyId {
    KeyId::new(Lane::Part(Part::Root), frame)
}

fn morph(frame: u32) -> Key {
    Key::Morph(MorphKey {
        morph: "あ".into(),
        frame,
        weight: 1.0,
        curve: VmdCurve::LINEAR,
    })
}

#[test]
fn shared_motion_is_copied_on_write() {
    let shared = Arc::new(Take::default());
    let mut edited = Arc::clone(&shared);
    let key = morph(3);
    Change::laying(&edited, vec![key.clone()]).apply(&mut edited);
    assert!(shared.morphs.is_empty());
    assert_eq!(edited.gather_keys(&FxHashSet::from_iter([key.id()])), [key]);
}

#[test]
fn pasted_keys_keep_spacing_and_tags() {
    let clip = Clip::lifted(vec![root(30, 1.0, Tag::Settle), morph(36), morph(45)]);
    assert!(clip.fits(|kind| matches!(kind, KeyKind::Part | KeyKind::Morph)));
    assert!(!clip.fits(|kind| kind == KeyKind::Morph));
    let pasted = clip.laid_at(4);
    assert_eq!(
        pasted.iter().map(Key::frame).collect::<Vec<_>>(),
        [4, 10, 19]
    );
    assert!(matches!(&pasted[0], Key::Part(key) if key.tag == Tag::Settle));
}

#[test]
fn part_keys_create_human_motion() {
    let mut edited = motion();
    let change = Change::laying(
        &edited,
        vec![
            root(4, 1.0, Tag::Smooth),
            Key::Bone(keyed("センター", 10, 9.0)),
        ],
    );
    change.apply(&mut edited);
    let human = edited.human.as_ref().expect("a human layer");
    assert_eq!(human.parts().keys(&Part::Root).len(), 1);
    assert!(bone_heights(&edited).contains(&("センター".into(), 10, 9.0)));

    let ids = FxHashSet::from_iter([part(4), id("センター", 10)]);
    assert_eq!(edited.gather_keys(&ids).len(), 2);

    change.revert(&mut edited);
    assert!(edited.human.as_ref().is_some_and(|human| human.is_empty()));
    assert_eq!(bone_heights(&edited), bone_heights(&motion()));
}

#[test]
fn morph_keys_lay_bend_and_revert() {
    let mut edited = motion();
    let change = Change::laying(&edited, vec![morph(3), morph(12)]);
    change.apply(&mut edited);
    assert_eq!(edited.morphs.keys("あ").len(), 2);

    let ids = FxHashSet::from_iter([KeyId::new(Lane::Morph("あ".into()), 12)]);
    let eased = VmdCurve {
        first: (64, 0),
        second: (64, 127),
    };
    let bent = Change::bending(edited.gather_keys(&ids), &Channel::OF_VALUES, eased);
    bent.apply(&mut edited);
    assert_eq!(
        edited.morphs.key("あ", 12).map(|key| key.curve),
        Some(eased)
    );
    assert_eq!(
        edited.morphs.key("あ", 3).map(|key| key.curve),
        Some(VmdCurve::LINEAR)
    );

    bent.revert(&mut edited);
    change.revert(&mut edited);
    assert!(edited.morphs.is_empty());
}
