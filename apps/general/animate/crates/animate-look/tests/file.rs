use std::sync::Arc;

use animate_look::{
    GraphRef, Look, LookFile, LookKey, LookValue, Name, SocketKey, Target, Texture,
};
use rikka_mmd::VmdCurve;
use rikka_renderer_3d::TextureKind;
use rikka_shader_graph::{NodeId, NodeKind, Scope, SocketRef};

use crate::support::{library, wired};

fn node(look: &Look, id: &str) -> NodeId {
    look.world
        .body()
        .nodes()
        .iter()
        .find(|(_, node)| node.kind == NodeKind::Extern(id.into()))
        .map(|(at, _)| *at)
        .expect("a node")
}

fn sample() -> Look {
    let mut look = Look {
        world: Arc::new(wired(&[
            (
                "world.panorama",
                vec![("image", "/home/me/scene/skies/dusk.hdr".into())],
            ),
            ("world.rig", vec![("figure", "miku".into())]),
        ])),
        ..Look::default()
    };
    let miku = Name::new("miku");
    look.slots.assign(miku, 0, Some("skin".into()));
    look.slots.set(
        miku,
        0,
        Name::new("ramp"),
        LookValue::Texture(Some(Texture::new(
            "/home/me/scene/ramps/warm.png",
            TextureKind::Ramp,
        ))),
    );
    let rig = node(&look, "world.rig");
    look.keys.apply(look.keys.inserting(LookKey {
        target: Target::Socket(SocketKey::new(
            GraphRef::World,
            SocketRef::new(Scope::Main, rig, "figure"),
        )),
        frame: 12,
        value: LookValue::Asset("rin".into()),
        curve: VmdCurve::LINEAR,
    }));
    look
}

fn relative(path: &str) -> String {
    path.strip_prefix("/home/me/scene/")
        .unwrap_or(path)
        .to_owned()
}

#[test]
fn look_files_round_trip_with_relative_assets() {
    let library = library();
    let look = sample();
    let mut saved = look.clone();
    saved.map_assets(library, relative).expect("maps");
    let text = serde_json::to_string_pretty(&LookFile::from_look(&saved)).expect("serializes");
    assert!(text.contains("\"skies/dusk.hdr\""), "{text}");
    assert!(text.contains("\"ramps/warm.png\""), "{text}");
    assert!(!text.contains("/home/me"), "{text}");
    assert!(
        text.contains("\"miku\"") && text.contains("\"rin\""),
        "names are not paths"
    );

    let file: LookFile = serde_json::from_str(&text).expect("deserializes");
    let mut loaded = file.look(library).expect("validates");
    assert_eq!(loaded, saved);
    loaded
        .map_assets(library, |path| format!("/home/me/scene/{path}"))
        .expect("maps back");
    assert_eq!(loaded, look);
}

#[test]
fn invalid_look_files_are_refused() {
    let library = library();
    let text = serde_json::to_string(&LookFile::from_look(&sample())).expect("serializes");
    let refused = [
        (
            "unknown node",
            text.replace("world.rig", "world.unheard_of"),
        ),
        ("swapped graphs", {
            let value: serde_json::Value = serde_json::from_str(&text).expect("json");
            let mut swapped = value.clone();
            swapped["world"] = value["composite"].clone();
            swapped.to_string()
        }),
        (
            "key on a missing input",
            text.replace("\"port\":\"figure\"", "\"port\":\"nonsense\""),
        ),
        (
            "text key on a number input",
            text.replace("\"port\":\"figure\"", "\"port\":\"azimuth\""),
        ),
        ("duplicate slot", {
            let mut value: serde_json::Value = serde_json::from_str(&text).expect("json");
            let slot = value["slots"][0].clone();
            value["slots"].as_array_mut().expect("slots").push(slot);
            value.to_string()
        }),
        ("duplicate key", {
            let mut value: serde_json::Value = serde_json::from_str(&text).expect("json");
            let key = value["keys"][0].clone();
            value["keys"].as_array_mut().expect("keys").push(key);
            value.to_string()
        }),
    ];
    for (case, text) in refused {
        let file: LookFile = serde_json::from_str(&text).expect("still a look file");
        assert!(file.look(library).is_err(), "{case} is accepted");
    }
}
