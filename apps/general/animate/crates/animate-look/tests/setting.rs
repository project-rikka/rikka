use animate_look::{GraphRef, Look, LookValue, Name, SlotKey, SocketKey, Target};
use rikka_mmd::VmdCurve;
use rikka_renderer_3d::{ParameterKind, TextureKind};
use rikka_shader_graph::{NodeKind, Scope, SocketRef, SocketValue};

use crate::support::library;

fn shadow_distance(look: &Look) -> Target {
    let shadows = look
        .world
        .body()
        .nodes()
        .iter()
        .find(|(_, node)| node.kind == NodeKind::Extern("world.shadows".into()))
        .map(|(id, _)| *id)
        .expect("the default world has shadows");
    Target::Socket(SocketKey::new(
        GraphRef::World,
        SocketRef::new(Scope::Main, shadows, "distance"),
    ))
}

#[test]
fn values_set_sockets_until_keyed_then_lay_keys() {
    let library = library();
    let mut look = Look::new(library).unwrap();
    let target = shadow_distance(&look);
    let Target::Socket(key) = &target else {
        unreachable!()
    };

    let undo = look
        .set_value(
            library,
            &target,
            Some(LookValue::Float(30.0)),
            5,
            VmdCurve::LINEAR,
        )
        .unwrap();
    assert_eq!(look.socket(key), Some(&SocketValue::from(30.0)));
    assert!(!look.is_keyed(&target));
    look.apply(library, undo).unwrap();
    assert_eq!(look.socket(key), None);

    look.toggle_key(
        library,
        &target,
        LookValue::Float(10.0),
        0,
        VmdCurve::LINEAR,
    )
    .unwrap();
    look.set_value(
        library,
        &target,
        Some(LookValue::Float(50.0)),
        20,
        VmdCurve::LINEAR,
    )
    .unwrap();
    assert_eq!(
        look.keys
            .tracks()
            .keys(&target)
            .iter()
            .map(|key| (key.frame, key.value.clone()))
            .collect::<Vec<_>>(),
        [(0, LookValue::Float(10.0)), (20, LookValue::Float(50.0))]
    );
    assert_eq!(
        look.value_at(library, &target, 10.0),
        Some(LookValue::Float(30.0))
    );

    look.toggle_key(
        library,
        &target,
        LookValue::Float(0.0),
        20,
        VmdCurve::LINEAR,
    )
    .unwrap();
    assert_eq!(look.keys.tracks().keys(&target).len(), 1);
}

#[test]
fn slot_values_rest_or_key() {
    let library = library();
    let mut look = Look::new(library).unwrap();
    let target = Target::Slot(SlotKey {
        figure: Name::new("miku"),
        material: 2,
        parameter: Name::new("tint"),
    });
    look.set_value(
        library,
        &target,
        Some(LookValue::Float(0.5)),
        0,
        VmdCurve::LINEAR,
    )
    .unwrap();
    assert_eq!(
        look.value_at(library, &target, 0.0),
        Some(LookValue::Float(0.5))
    );
    look.toggle_key(library, &target, LookValue::Float(0.8), 4, VmdCurve::LINEAR)
        .unwrap();
    look.set_value(
        library,
        &target,
        Some(LookValue::Float(0.2)),
        4,
        VmdCurve::LINEAR,
    )
    .unwrap();
    assert_eq!(
        look.value_at(library, &target, 4.0),
        Some(LookValue::Float(0.2))
    );
    look.set_value(library, &target, None, 4, VmdCurve::LINEAR)
        .unwrap();
    assert!(
        look.slots
            .slot(Name::new("miku"), 2)
            .is_none_or(|slot| slot.values.is_empty())
    );
}

#[test]
fn parameter_values_convert_through_sockets() {
    let options = ParameterKind::Enum {
        options: vec!["flat".into(), "soft".into(), "hard".into()],
    };
    let chosen = LookValue::from_parameter(&options, &SocketValue::from(1.0)).unwrap();
    assert_eq!(chosen, LookValue::Enum(Name::new("soft")));
    assert_eq!(
        chosen.to_parameter_socket(&options),
        Some(SocketValue::from(1.0))
    );
    let texture = ParameterKind::Texture {
        kind: TextureKind::Normal,
        sampler: true,
    };
    let picked = LookValue::from_parameter(&texture, &SocketValue::from("face.png")).unwrap();
    assert!(matches!(&picked, LookValue::Texture(Some(held)) if held.kind == TextureKind::Normal));
    assert_eq!(
        picked.to_parameter_socket(&texture),
        Some(SocketValue::from("face.png"))
    );
}
