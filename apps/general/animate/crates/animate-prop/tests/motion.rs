use animate_prop::{Anchor, AnchorKey, PropLevels, PropMotion, PropSetting};
use rikka_model_core::glam::{Vec3, Vec4Swizzles};

use crate::key;

fn anchor(figure: &str, bone: &str) -> Option<Anchor> {
    Some(Anchor {
        figure: figure.to_owned(),
        bone: bone.to_owned(),
    })
}

#[test]
fn placement_scales_then_turns_then_moves() {
    let levels = PropLevels {
        x: 1.0,
        y: 2.0,
        yaw: 90.0,
        scale: 2.0,
        ..PropLevels::default()
    };
    let moved = levels.transform() * Vec3::NEG_Z.extend(1.0);
    assert!(
        moved.xyz().abs_diff_eq(Vec3::new(-1.0, 2.0, 0.0), 1e-5),
        "a positive yaw turns the front counter-clockwise seen from above: {moved}"
    );
    let pitched = PropLevels {
        pitch: 90.0,
        ..PropLevels::default()
    };
    assert!(
        (pitched.transform() * Vec3::NEG_Z.extend(0.0))
            .xyz()
            .abs_diff_eq(Vec3::Y, 1e-5),
        "a positive pitch lifts the front"
    );
}

#[test]
fn keyed_settings_interpolate_and_visibility_holds() {
    let mut motion = PropMotion::default();
    assert!(motion.is_empty());
    motion.levels.y = 0.5;
    motion.tracks_mut().insert(key(PropSetting::X, 0, 0.0));
    motion.tracks_mut().insert(key(PropSetting::X, 10, 10.0));
    motion
        .tracks_mut()
        .insert(key(PropSetting::Visible, 0, 1.0));
    motion
        .tracks_mut()
        .insert(key(PropSetting::Visible, 10, 0.0));
    let between = motion.levels_at(5.0);
    assert!((between.x - 5.0).abs() < 1e-4, "{}", between.x);
    assert_eq!(between.y, 0.5, "unkeyed settings rest");
    assert!(between.visible, "visibility holds until its next key");
    assert!(!motion.levels_at(10.0).visible);
    assert_eq!(motion.last_frame(), 10);
}

#[test]
fn attachment_follows_the_key_held_at_the_frame() {
    let mut motion = PropMotion::default();
    motion.anchor = anchor("Miku", "Right wrist");
    assert_eq!(
        motion.anchor_at(30.0),
        anchor("Miku", "Right wrist").as_ref()
    );
    motion.anchors_mut().insert(AnchorKey {
        frame: 20,
        anchor: anchor("Miku", "Head"),
    });
    motion.anchors_mut().insert(AnchorKey {
        frame: 40,
        anchor: None,
    });
    assert_eq!(motion.anchor_at(0.0), anchor("Miku", "Head").as_ref());
    assert_eq!(motion.anchor_at(39.9), anchor("Miku", "Head").as_ref());
    assert_eq!(
        motion.anchor_at(40.0),
        None,
        "a key may drop the attachment"
    );
    assert_eq!(motion.last_frame(), 40);
}
