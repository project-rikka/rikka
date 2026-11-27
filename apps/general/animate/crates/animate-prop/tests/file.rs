use animate_prop::{Anchor, AnchorKey, Error, PropFile, PropKey, PropMotion, PropSetting, Result};
use rikka_mmd::VmdCurve;

use crate::key;

fn placed() -> PropMotion {
    let mut motion = PropMotion::default();
    motion.levels.z = -3.0;
    motion.levels.scale = 1.5;
    motion.anchor = Some(Anchor {
        figure: "Miku".to_owned(),
        bone: "Right wrist".to_owned(),
    });
    motion.tracks_mut().insert(key(PropSetting::Yaw, 0, -45.0));
    motion.tracks_mut().insert(PropKey {
        curve: VmdCurve {
            first: (64, 3),
            second: (90, 127),
        },
        ..key(PropSetting::Yaw, 48, 300.0)
    });
    motion
        .tracks_mut()
        .insert(key(PropSetting::Visible, 60, 0.0));
    motion.anchors_mut().insert(AnchorKey {
        frame: 30,
        anchor: None,
    });
    motion
}

fn edited(change: impl FnOnce(&mut serde_json::Value)) -> Result<PropMotion> {
    let mut text = serde_json::to_value(PropFile::from_motion(&placed())).expect("writes");
    change(&mut text);
    let file: PropFile = serde_json::from_value(text).expect("well formed");
    file.motion()
}

#[test]
fn prop_file_round_trip() {
    let motion = placed();
    let text = serde_json::to_string(&PropFile::from_motion(&motion)).expect("a prop writes");
    assert!(text.contains(r#""scale":1.5"#), "{text}");
    let back: PropFile = serde_json::from_str(&text).expect("reads back");
    assert_eq!(back.motion().expect("into a prop motion"), motion);
    let still = PropFile::from_motion(&PropMotion::default());
    assert_eq!(still.motion().expect("a still prop"), PropMotion::default());
}

#[test]
fn rejects_invalid_settings_and_attachments() {
    let unknown = edited(|text| text["keys"][0]["setting"] = "tilt".into());
    assert!(matches!(unknown, Err(Error::UnknownSetting(id)) if id == "tilt"));
    let level = edited(|text| text["levels"]["scale"] = 0.0.into());
    assert!(matches!(level, Err(Error::OutOfRange(id)) if id == "scale"));
    let shown = edited(|text| text["keys"][2]["value"] = 0.5.into());
    assert!(matches!(shown, Err(Error::InvalidKey { frame: 60, .. })));
    let twice = edited(|text| {
        let first = text["keys"][0].clone();
        text["keys"].as_array_mut().expect("keys").push(first);
    });
    assert!(matches!(twice, Err(Error::DuplicateKey { frame: 0, .. })));
    let nameless = edited(|text| text["anchor"]["bone"] = "".into());
    assert!(matches!(nameless, Err(Error::InvalidAnchor(0))));
    let again = edited(|text| {
        let first = text["anchors"][0].clone();
        text["anchors"].as_array_mut().expect("anchors").push(first);
    });
    assert!(matches!(again, Err(Error::DuplicateAnchor(30))));
}
