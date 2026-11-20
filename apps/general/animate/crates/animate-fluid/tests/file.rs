use animate_fluid::{
    Aim, Anchor, Emitter, EmitterFile, EmitterId, EmitterKey, EmitterSetting, EmitterValue, Error,
    Pour, Preset, Property, Result, Shape,
};
use rikka_mmd::VmdCurve;
use rikka_model_core::glam::Vec3;

use crate::support::key;

fn lotion_emitter() -> Emitter {
    let mut emitter = Emitter::new(EmitterId(7), "lotion on the palm", Shape::blob());
    emitter.aim = Aim::Direction;
    emitter.launch.anchor = Anchor::bone("miku", "右手首");
    emitter.launch.offset = Vec3::new(0.0, 0.1, 0.02);
    emitter.target.anchor = Anchor::bone("miku", "左手首");
    emitter.levels.volume = 0.03;
    emitter.liquid.choose(Preset::Lotion);
    emitter
        .liquid
        .set(Property::Color, EmitterValue::Three([1.0, 0.7, 0.85]));
    emitter
        .liquid
        .set(Property::Viscosity, EmitterValue::Number(0.65));
    emitter.pour = Pour::new(30, 45);
    emitter.tracks_mut().insert(EmitterKey {
        curve: VmdCurve {
            first: (64, 3),
            second: (90, 127),
        },
        ..key(EmitterSetting::Volume, 30, EmitterValue::Number(0.04))
    });
    emitter.tracks_mut().insert(key(
        EmitterSetting::Target,
        45,
        EmitterValue::Three([0.0, -0.05, 0.1]),
    ));
    emitter
}

fn edited(change: impl FnOnce(&mut serde_json::Value)) -> Result<Emitter> {
    let mut text =
        serde_json::to_value(EmitterFile::from_emitter(&lotion_emitter())).expect("writes");
    change(&mut text);
    let file: EmitterFile = serde_json::from_value(text).expect("well formed");
    file.emitter(EmitterId(0))
}

#[test]
fn emitter_file_round_trip() {
    let emitter = lotion_emitter();
    let text = serde_json::to_string(&EmitterFile::from_emitter(&emitter)).expect("writes");
    assert!(text.contains(r#""preset":"lotion""#), "{text}");
    assert!(text.contains(r#""type":"blob""#), "{text}");
    let back: EmitterFile = serde_json::from_str(&text).expect("reads back");
    let mut read = back.emitter(EmitterId(9)).expect("into an emitter");
    assert_eq!(read.id, EmitterId(9));
    read.id = emitter.id;
    assert_eq!(read, emitter);
}

#[test]
fn shapes_and_presets_round_trip() {
    let shapes = [
        Shape::Stream,
        Shape::Shower,
        Shape::Drip,
        Shape::Blob { size: 0.2 },
        Shape::surface("rin", [3, 1]),
    ];
    for (shape, preset) in shapes.into_iter().zip(Preset::ALL.into_iter().cycle()) {
        let mut emitter = Emitter::new(EmitterId(0), "any", shape);
        emitter.liquid.choose(preset);
        let text = serde_json::to_string(&EmitterFile::from_emitter(&emitter)).expect("writes");
        let back: EmitterFile = serde_json::from_str(&text).expect("reads");
        assert_eq!(back.emitter(EmitterId(0)).expect("an emitter"), emitter);
    }
    for preset in Preset::ALL {
        let mut emitter = Emitter::new(EmitterId(0), "any", Shape::Stream);
        emitter.liquid.choose(preset);
        let back = EmitterFile::from_emitter(&emitter)
            .emitter(EmitterId(0))
            .expect("an emitter");
        assert_eq!(back.liquid.liquid(), preset.liquid());
        assert_eq!(back.liquid.overrides().count(), 0);
    }
}

#[test]
fn rejects_unknown_names_or_out_of_range_values() {
    let unknown = edited(|text| text["keys"][0]["setting"] = "glitter".into());
    assert!(matches!(unknown, Err(Error::UnknownSetting(id)) if id == "glitter"));
    let preset = edited(|text| text["liquid"]["preset"] = "lava".into());
    assert!(matches!(preset, Err(Error::UnknownPreset(id)) if id == "lava"));
    let property = edited(|text| text["liquid"]["overrides"]["glow"] = 1.0.into());
    assert!(matches!(property, Err(Error::UnknownProperty(id)) if id == "glow"));
    let aim = edited(|text| text["aim"] = "homing".into());
    assert!(matches!(aim, Err(Error::UnknownAim(id)) if id == "homing"));
    let invalid = edited(|text| text["keys"][0]["value"] = 3.0.into());
    assert!(matches!(invalid, Err(Error::InvalidKey { frame: 30, .. })));
    let level = edited(|text| text["levels"]["rate"] = 40.0.into());
    assert!(matches!(level, Err(Error::OutOfRange { name }) if name == "rate"));
    let colour = edited(|text| {
        text["liquid"]["overrides"]["color"] = serde_json::json!([2.0, 0.0, 0.0]);
    });
    assert!(matches!(colour, Err(Error::OutOfRange { name }) if name == "color"));
    let size = edited(|text| text["shape"]["size"] = 9.0.into());
    assert!(matches!(size, Err(Error::OutOfRange { .. })));
    for (field, value) in [("length", 0), ("length", u32::MAX), ("start", u32::MAX)] {
        let pour = edited(|text| text["pour"][field] = value.into());
        assert!(
            matches!(pour, Err(Error::OutOfRange { name }) if name == "pour"),
            "{field}"
        );
    }
    let twice = edited(|text| {
        let first = text["keys"][0].clone();
        text["keys"].as_array_mut().expect("keys").push(first);
    });
    assert!(matches!(twice, Err(Error::DuplicateKey { frame: 30, .. })));
}
