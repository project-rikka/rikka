use animate_wind::{Error, Result, WindFile, WindKey, WindLevels, WindMotion, WindSetting};
use rikka_mmd::VmdCurve;

use crate::support::key;

fn breeze() -> WindMotion {
    let mut motion = WindMotion::default();
    motion.levels.speed = 4.5;
    motion.levels.pitch = 10.0;
    motion.tracks_mut().insert(key(WindSetting::Yaw, 0, -120.0));
    motion.tracks_mut().insert(WindKey {
        curve: VmdCurve {
            first: (64, 3),
            second: (90, 127),
        },
        ..key(WindSetting::Yaw, 48, 150.0)
    });
    motion
        .tracks_mut()
        .insert(key(WindSetting::Turbulence, 12, 0.8));
    motion
}

fn edited(change: impl FnOnce(&mut serde_json::Value)) -> Result<WindMotion> {
    let mut text = serde_json::to_value(WindFile::from_motion(&breeze())).expect("writes");
    change(&mut text);
    let file: WindFile = serde_json::from_value(text).expect("well formed");
    file.motion()
}

#[test]
fn wind_file_round_trip() {
    let motion = breeze();
    let text = serde_json::to_string(&WindFile::from_motion(&motion)).expect("a wind writes");
    assert!(text.contains(r#""speed":4.5"#), "{text}");
    let back: WindFile = serde_json::from_str(&text).expect("reads back");
    assert_eq!(back.motion().expect("into a wind"), motion);
    let calm = WindFile::from_motion(&WindMotion::default());
    assert_eq!(calm.motion().expect("a calm wind"), WindMotion::default());
}

#[test]
fn rejects_unknown_or_out_of_range_settings() {
    let unknown = edited(|text| text["keys"][0]["setting"] = "drizzle".into());
    assert!(matches!(unknown, Err(Error::UnknownSetting(id)) if id == "drizzle"));
    let level = edited(|text| text["levels"]["speed"] = 400.0.into());
    assert!(matches!(level, Err(Error::OutOfRange(id)) if id == "speed"));
    let invalid = edited(|text| text["keys"][0]["value"] = 270.0.into());
    assert!(matches!(invalid, Err(Error::InvalidKey { frame: 0, .. })));
    let twice = edited(|text| {
        let first = text["keys"][0].clone();
        text["keys"].as_array_mut().expect("keys").push(first);
    });
    assert!(matches!(twice, Err(Error::DuplicateKey { frame: 0, .. })));
    let sparse = edited(|text| {
        text["levels"]
            .as_object_mut()
            .expect("levels")
            .remove("gust");
    });
    assert_eq!(
        sparse.expect("missing levels rest").levels.gust,
        WindLevels::default().gust
    );
}
