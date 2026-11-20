use animate_document::{Error, Motion};
use animate_fluid::{Emitter, EmitterId, EmitterKey, Preset, Shape};
use animate_look::{GraphRef, Look, SocketKey, shared_library};
use animate_motion::{MorphKey, Take};
use animate_wind::{WindKey, WindMotion};
use rikka_mmd::{VmdCameraFrame, VmdCameraInterpolation, VmdCurve};
use rikka_model_core::glam::Vec3;
use rikka_shader_graph::{NodeKind, Scope, SocketRef};

const BENT: VmdCurve = VmdCurve {
    first: (64, 3),
    second: (90, 127),
};

fn json(motion: &Motion) -> String {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join(format!("take.{}", Motion::EXTENSION));
    motion.save(&path).expect("a motion saves");
    std::fs::read_to_string(&path).expect("the saved file reads")
}

fn read(json: impl AsRef<[u8]>) -> Result<Motion, Error> {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join(format!("take.{}", Motion::EXTENSION));
    std::fs::write(&path, json).expect("the file writes");
    Motion::load(&path)
}

fn with(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from} in {text}");
    text.replacen(from, to, 1)
}

fn figure() -> Motion {
    Motion::Figure(Box::new(Take {
        morphs: [MorphKey {
            morph: "あ".into(),
            frame: 9,
            weight: 0.3,
            curve: BENT,
        }]
        .into_iter()
        .collect(),
        ..Take::default()
    }))
}

#[test]
fn every_kind_round_trips() {
    let mut look = Look::default();
    let backdrop = look
        .world
        .body()
        .nodes()
        .iter()
        .find(|(_, node)| node.kind == NodeKind::Extern("world.backdrop".into()))
        .map(|(id, _)| *id)
        .expect("the default world has a backdrop");
    look.key_socket(
        shared_library(),
        SocketKey::new(
            GraphRef::World,
            SocketRef::new(Scope::Main, backdrop, "color"),
        ),
        12,
        [0.1, 0.2, 0.3].into(),
        BENT,
    )
    .expect("keys the backdrop");
    let mut wind = WindMotion::default();
    wind.levels.speed = 6.0;
    wind.tracks_mut().insert(WindKey {
        setting: animate_wind::WindSetting::Yaw,
        frame: 30,
        value: 135.0,
        curve: BENT,
    });
    let mut emitter = Emitter::new(EmitterId(0), "hose", Shape::Shower);
    emitter.liquid.choose(Preset::Honey);
    emitter.tracks_mut().insert(EmitterKey {
        setting: animate_fluid::EmitterSetting::Spread,
        frame: 24,
        value: animate_fluid::EmitterValue::Number(45.0),
        curve: BENT,
    });
    let camera = Motion::Camera(Box::new(
        [VmdCameraFrame {
            frame: 30,
            distance: -4.5,
            target: Vec3::new(0.0, 1.2, 0.0),
            rotation: Vec3::new(0.1, -0.2, 0.0),
            interpolation: VmdCameraInterpolation {
                distance: BENT,
                ..VmdCameraInterpolation::default()
            },
            angle: 27,
            perspective: false,
        }]
        .into_iter()
        .collect(),
    ));
    let kinds = [
        ("figure", figure()),
        ("camera", camera),
        ("look", Motion::Look(Box::new(look))),
        ("wind", Motion::Wind(Box::new(wind))),
        ("fluid", Motion::Fluid(Box::new(emitter))),
    ];
    for (kind, motion) in kinds {
        let text = json(&motion);
        assert!(text.contains(&format!("\"kind\": \"{kind}\"")), "{text}");
        assert_eq!(read(&text).ok(), Some(motion), "{kind}");
    }
}

#[test]
fn rejects_unknown_version_format_or_kind() {
    let text = json(&figure());
    assert!(matches!(
        read(with(&text, "\"version\": 1", "\"version\": 2")),
        Err(Error::File(rikka_file_format::Error::Decode(
            rikka_file_format::DecodeError::Version { found: 2, .. }
        )))
    ));
    assert!(matches!(
        read(with(&text, "rikka-motion", "other-motion")),
        Err(Error::File(rikka_file_format::Error::Decode(
            rikka_file_format::DecodeError::Foreign { .. }
        )))
    ));
    assert!(matches!(
        read(with(&text, "\"figure\"", "\"light\"")),
        Err(Error::File(rikka_file_format::Error::Json { .. }))
    ));
    assert!(matches!(
        read("{\"format\":"),
        Err(Error::File(rikka_file_format::Error::Json { .. }))
    ));
}

#[test]
fn reports_errors_of_each_kind() {
    let mut wind = WindMotion::default();
    wind.levels.speed = 6.0;
    let wind = json(&Motion::Wind(Box::new(wind)));
    assert!(matches!(
        read(with(&wind, "\"speed\": 6.0", "\"speed\": -1.0")),
        Err(Error::Wind(animate_wind::Error::OutOfRange(_)))
    ));
    let mut emitter = Emitter::new(EmitterId(0), "hose", Shape::Shower);
    emitter.liquid.choose(Preset::Honey);
    let fluid = json(&Motion::Fluid(Box::new(emitter)));
    assert!(matches!(
        read(with(&fluid, "\"honey\"", "\"lava\"")),
        Err(Error::Fluid(animate_fluid::Error::UnknownPreset(_)))
    ));
    let figure = json(&figure());
    assert!(matches!(
        read(with(&figure, "\"weight\": 0.3", "\"weight\": 1e39")),
        Err(Error::Take(animate_motion::Error::Malformed {
            frame: 9,
            ..
        }))
    ));
}
