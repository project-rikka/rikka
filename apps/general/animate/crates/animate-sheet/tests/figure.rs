use std::sync::Arc;

use animate_body::{Faces, Panel, Side};
use animate_hold::{Hold, HoldTarget, Holder};
use animate_motion::{HumanMotion, MorphKey, Morphs, Part, PartKey, PartValues, Tag, Take};
use animate_sheet::{BoneDisplay, GroupKind, Listing, Sheet};
use rikka_mmd::{
    Pmx, PmxBone, PmxDisplayFrame, PmxDisplayTarget, PmxMorph, PmxOffsets, PmxPanel, VmdBoneFrame,
    VmdCurve, VmdInterpolation, VmdPropertyFrame,
};
use rikka_model_core::glam::{Quat, Vec3};
use rikka_model_core::{ModelBuilder, Node};

fn bone(name: &str) -> PmxBone {
    PmxBone {
        name: name.into(),
        ..PmxBone::default()
    }
}

fn morph(name: &str, panel: PmxPanel) -> PmxMorph {
    PmxMorph {
        name: name.into(),
        english_name: String::new(),
        panel,
        offsets: PmxOffsets::Group(Vec::new()),
    }
}

fn keyed(bone: &str, frame: u32) -> VmdBoneFrame {
    VmdBoneFrame {
        bone: bone.into(),
        frame,
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        interpolation: VmdInterpolation::default(),
        physics_off: false,
    }
}

fn pmx() -> Pmx {
    Pmx {
        bones: vec![bone("センター"), bone("首"), bone("操作中心")],
        morphs: vec![
            morph("あ", PmxPanel::Mouth),
            morph("まばたき", PmxPanel::Eye),
            morph("隠し", PmxPanel::Hidden),
            morph("内部", PmxPanel::Hidden),
        ],
        display_frames: vec![
            PmxDisplayFrame {
                name: "センター".into(),
                targets: vec![PmxDisplayTarget::Bone(0)],
                ..PmxDisplayFrame::default()
            },
            PmxDisplayFrame {
                name: "表情".into(),
                special: true,
                targets: vec![PmxDisplayTarget::Morph(0), PmxDisplayTarget::Morph(1)],
                ..PmxDisplayFrame::default()
            },
            PmxDisplayFrame {
                name: "空".into(),
                ..PmxDisplayFrame::default()
            },
        ],
        ..Pmx::default()
    }
}

fn faces() -> Faces {
    let mut builder = ModelBuilder::new();
    builder.add_node(Node::new("root"));
    let mut model = builder.finish().expect("one node is a model");
    model.extensions.insert(pmx());
    Faces::from_model(&model)
}

fn motion() -> Take {
    Take {
        bones: [
            keyed("センター", 30),
            keyed("センター", 0),
            keyed("首", 15),
            keyed("しっぽ", 5),
        ]
        .into_iter()
        .collect(),
        switches: [VmdPropertyFrame {
            frame: 0,
            visible: true,
            ik: Vec::new(),
        }]
        .into_iter()
        .collect(),
        ..Take::default()
    }
}

fn take() -> Take {
    Take {
        morphs: morphs(),
        human: Some(HumanMotion::default()),
        ..motion()
    }
}

fn everything(_: &str) -> bool {
    true
}

fn undriven(name: &str) -> bool {
    name != "センター"
}

fn morphs() -> Morphs {
    [("あ", 12), ("隠し", 3), ("ないもの", 40)]
        .into_iter()
        .map(|(morph, frame)| MorphKey {
            morph: morph.into(),
            frame,
            weight: 1.0,
            curve: VmdCurve::LINEAR,
        })
        .collect()
}

fn kinds(sheet: &Sheet) -> Vec<GroupKind> {
    sheet
        .groups()
        .iter()
        .map(|group| group.kind.clone())
        .collect()
}

fn names(sheet: &Sheet, at: usize) -> Vec<String> {
    sheet.groups()[at]
        .tracks
        .iter()
        .map(|track| track.name().to_owned())
        .collect()
}

#[test]
fn sheet_groups_by_display_frame_and_panel() {
    let display = BoneDisplay::from_pmx(&pmx());
    let listing = Listing::Bones {
        display: &display,
        shown: &everything,
    };
    let sheet = Sheet::of_take(listing, &faces(), &take());
    assert_eq!(
        kinds(&sheet),
        [
            GroupKind::Switches,
            GroupKind::Listed("センター".into()),
            GroupKind::Unlisted,
            GroupKind::Morphs(Panel::Eye),
            GroupKind::Morphs(Panel::Mouth),
            GroupKind::Morphs(Panel::Other),
            GroupKind::Unmatched,
        ]
    );
    assert_eq!(sheet.groups()[0].keys, [0]);
    assert_eq!(sheet.groups()[1].tracks[0].keys, [0, 30]);
    assert_eq!(names(&sheet, 2), ["首"], "keyed but on no display frame");
    assert_eq!(names(&sheet, 3), ["まばたき"], "listed though never keyed");
    assert_eq!(sheet.groups()[4].tracks[0].keys, [12]);
    assert_eq!(
        names(&sheet, 5),
        ["隠し"],
        "a hidden morph shows once keyed"
    );
    assert_eq!(
        names(&sheet, 6),
        ["しっぽ", "ないもの"],
        "keyed for a bone or morph the model lacks"
    );
    assert_eq!(sheet.groups()[6].keys, [5, 40]);
    assert_eq!(sheet.keys(), [0, 3, 5, 12, 15, 30, 40]);
    assert_eq!(sheet.next_key(12), Some(15));
    assert_eq!(sheet.next_key(13), Some(15));
    assert_eq!(sheet.next_key(40), None);
    assert_eq!(sheet.previous_key(15), Some(12));
    assert_eq!(sheet.previous_key(0), None);
    assert_eq!(sheet.last_key(), Some(40));
}

#[test]
fn detail_sheet_omits_driven_bones() {
    let display = BoneDisplay::from_pmx(&pmx());
    let listing = Listing::Bones {
        display: &display,
        shown: &undriven,
    };
    let sheet = Sheet::of_take(listing, &faces(), &take());
    assert_eq!(
        kinds(&sheet),
        [
            GroupKind::Switches,
            GroupKind::Unlisted,
            GroupKind::Morphs(Panel::Eye),
            GroupKind::Morphs(Panel::Mouth),
            GroupKind::Morphs(Panel::Other),
            GroupKind::Unmatched,
        ]
    );
    assert_eq!(sheet.keys(), [0, 3, 5, 12, 15, 40]);
}

type Edit = fn(&mut Take);

fn morph_key(morph: &str, frame: u32) -> MorphKey {
    MorphKey {
        morph: morph.into(),
        frame,
        weight: 0.5,
        curve: VmdCurve::LINEAR,
    }
}

fn root_key(frame: u32) -> PartKey {
    PartKey {
        part: Part::Root,
        frame,
        tag: Tag::Ease,
        values: PartValues::Root {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        },
        slopes: None,
    }
}

#[test]
fn retaken_sheet_matches_a_fresh_one() {
    let edits: [(&str, Edit); 10] = [
        ("key a listed bone", |take| {
            take.bones.insert(keyed("センター", 45));
        }),
        ("unkey a listed bone", |take| {
            take.bones.remove("センター", 0);
            take.bones.remove("センター", 30);
        }),
        ("unkey an unlisted bone", |take| {
            take.bones.remove("首", 15);
        }),
        ("key a bone the model lacks", |take| {
            take.bones.insert(keyed("尻尾2", 7));
        }),
        ("key a listed morph", |take| {
            take.morphs.insert(morph_key("あ", 20));
        }),
        ("unkey a hidden morph", |take| {
            take.morphs.remove("隠し", 3);
        }),
        ("key a never keyed hidden morph", |take| {
            take.morphs.insert(morph_key("内部", 9));
        }),
        ("key the switches", |take| {
            take.switches.insert(VmdPropertyFrame {
                frame: 10,
                visible: false,
                ik: Vec::new(),
            });
        }),
        ("key a part", |take| {
            if let Some(human) = &mut take.human {
                human.insert(root_key(8));
            }
        }),
        ("lay a hold", |take| {
            take.holds.lay(Hold {
                until: Some(9),
                ..Hold::new(
                    Holder::Hand(Side::Left),
                    HoldTarget {
                        figure: None,
                        at: Vec3::ZERO,
                        reach: 0.1,
                    },
                    4,
                )
            });
        }),
    ];
    let display = BoneDisplay::from_pmx(&pmx());
    let faces = faces();
    let listings = [
        Listing::Bones {
            display: &display,
            shown: &everything,
        },
        Listing::Bones {
            display: &display,
            shown: &undriven,
        },
        Listing::Parts,
        Listing::Morphs,
    ];
    for listing in listings {
        for (name, edit) in edits {
            let before = take();
            let mut after = before.clone();
            edit(&mut after);
            let shown = Sheet::of_take(listing, &faces, &before);
            assert_eq!(
                shown.retaken(listing, &faces, &before, &after),
                Sheet::of_take(listing, &faces, &after),
                "{name} in {listing:?}"
            );
        }
    }
}

#[test]
fn retaking_keeps_untouched_groups() {
    let display = BoneDisplay::from_pmx(&pmx());
    let faces = faces();
    let listing = Listing::Bones {
        display: &display,
        shown: &everything,
    };
    let before = take();
    let mut after = before.clone();
    after.bones.insert(keyed("センター", 45));
    let shown = Sheet::of_take(listing, &faces, &before);
    let retaken = shown.retaken(listing, &faces, &before, &after);
    let shared: Vec<bool> = shown
        .groups()
        .iter()
        .zip(retaken.groups())
        .map(|(one, two)| Arc::ptr_eq(one, two))
        .collect();
    assert_eq!(shared, [true, false, true, true, true, true, true]);
    assert_eq!(retaken.keys(), [0, 3, 5, 12, 15, 30, 40, 45]);
}
