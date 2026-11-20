use std::sync::Arc;

use animate_look::world::Backdrop;
use animate_look::{
    GraphRef, KeyEdit, Keys, Look, LookEdit, LookKey, LookValue, Name, SlotKey, SocketKey, Target,
};
use rikka_mmd::VmdCurve;
use rikka_renderer_3d::glam::Vec3;
use rikka_shader_graph::{
    Builder, Edit, GraphKind, Input, NodeKind, Op, Scope, SocketRef, SocketValue,
};

use crate::support::{evaluated_world, library, wired, world_at};

fn key(target: &Target, frame: u32, value: LookValue) -> LookKey {
    LookKey {
        target: target.clone(),
        frame,
        value,
        curve: VmdCurve::LINEAR,
    }
}

fn socket(graph: GraphRef, node: u32, port: &str) -> Target {
    Target::Socket(SocketKey::new(
        graph,
        SocketRef::new(Scope::Main, node_id(node), port),
    ))
}

fn node_id(id: u32) -> rikka_shader_graph::NodeId {
    serde_json::from_value(serde_json::json!(id)).expect("a node id")
}

fn backdrop_node(look: &Look) -> rikka_shader_graph::NodeId {
    look.world
        .body()
        .nodes()
        .iter()
        .find(|(_, node)| node.kind == NodeKind::Extern("world.backdrop".into()))
        .map(|(id, _)| *id)
        .expect("the default world has a backdrop")
}

#[test]
fn key_edits_undo_and_sample() {
    let mut keys = Keys::default();
    let hour = socket(GraphRef::World, 3, "hour");
    let name = socket(GraphRef::World, 4, "figure");
    let first = keys.inserting(key(&hour, 0, LookValue::Float(6.0)));
    let undo_first = keys.apply(first);
    keys.apply(keys.inserting(key(&hour, 10, LookValue::Float(18.0))));
    keys.apply(keys.inserting(key(&name, 0, LookValue::Asset("miku".into()))));
    keys.apply(keys.inserting(key(&name, 10, LookValue::Asset("rin".into()))));

    let sampled = keys.sample(5.0);
    let Target::Socket(hour_key) = &hour else {
        unreachable!()
    };
    let Target::Socket(name_key) = &name else {
        unreachable!()
    };
    assert_eq!(sampled.socket(hour_key), Some(&SocketValue::from(12.0)));
    assert_eq!(
        sampled.socket(name_key),
        Some(&SocketValue::from("miku")),
        "assets step"
    );

    let before = keys.clone();
    let moved = keys.apply(keys.moving(&hour, 10, 20));
    assert_eq!(
        keys.tracks()
            .keys(&hour)
            .iter()
            .map(|key| key.frame)
            .collect::<Vec<_>>(),
        [0, 20]
    );
    keys.apply(moved);
    assert_eq!(keys, before);

    let elsewhere = socket(GraphRef::Composite, 1, "hour");
    let retargeted = keys.apply(keys.retargeting(&hour, &elsewhere));
    assert!(keys.tracks().keys(&hour).is_empty());
    assert_eq!(keys.tracks().keys(&elsewhere).len(), 2);
    keys.apply(retargeted);
    assert_eq!(keys, before);

    let removed = keys.apply(keys.removing(&name, 0));
    assert_eq!(keys.tracks().keys(&name).len(), 1);
    keys.apply(removed);
    assert_eq!(keys, before);

    keys.apply(KeyEdit {
        removed: before.tracks().all_keys().cloned().collect(),
        added: Vec::new(),
    });
    keys.apply(undo_first.inverse());
    assert_eq!(keys.tracks().keys(&hour).len(), 1);
}

#[test]
fn choices_hold_and_slot_values_blend() {
    let mut keys = Keys::default();
    let tone = socket(GraphRef::Composite, 2, "tone");
    let slot = SlotKey {
        figure: Name::new("miku"),
        material: 3,
        parameter: Name::new("rim"),
    };
    for (frame, choice, rim) in [(0, 0, 0.0), (10, 4, 1.0)] {
        keys.apply(keys.inserting(key(&tone, frame, LookValue::Choice(choice))));
        keys.apply(keys.inserting(key(&Target::Slot(slot), frame, LookValue::Float(rim))));
    }
    let sampled = keys.sample(9.0);
    let Target::Socket(tone_key) = &tone else {
        unreachable!()
    };
    assert_eq!(sampled.socket(tone_key), Some(&SocketValue::from(0.0)));
    assert_eq!(sampled.slot(&slot), Some(&LookValue::Float(0.9)));
}

#[test]
fn time_and_keys_drive_world_sockets() {
    let library = library();
    let mut builder = Builder::new(GraphKind::World, library);
    let output = builder.node(NodeKind::Output);
    let time = builder.input(Input::Time);
    let hour = builder.op(Op::Multiply, [time.into(), 2.0.into()]);
    let sky = builder.external("world.sky", [("hour", hour.into())]);
    let wire = builder.out(sky, "environment");
    builder.set(output, "environment", wire);
    let graph = builder.finish().expect("a world");
    assert!(matches!(
        world_at(&graph, 3.0).backdrop,
        Some(Backdrop::Sky(sky)) if sky.hour == 6.0
    ));

    let mut look = Look::default();
    let backdrop = backdrop_node(&look);
    let color = socket(GraphRef::World, backdrop.get(), "color");
    look.keys.apply(
        look.keys
            .inserting(key(&color, 0, LookValue::Vec3([0.0, 0.0, 0.0]))),
    );
    look.keys.apply(
        look.keys
            .inserting(key(&color, 10, LookValue::Vec3([1.0, 0.5, 0.0]))),
    );
    assert_eq!(
        evaluated_world(&look, 5.0).backdrop,
        Some(Backdrop::Color(Vec3::new(0.5, 0.25, 0.0))),
        "a keyed socket drives even when the graph leaves it unset"
    );

    let mut keyed = Look::default();
    let Target::Socket(at) = &color else {
        unreachable!()
    };
    keyed
        .key_socket(
            library,
            at.clone(),
            0,
            [0.2, 0.4, 0.6].into(),
            VmdCurve::LINEAR,
        )
        .expect("keys the socket");
    assert_eq!(keyed.socket(at), Some(&SocketValue::from([0.2, 0.4, 0.6])));
}

#[test]
fn removing_a_node_drops_its_tracks_until_undone() {
    let library = library();
    let mut look = Look::default();
    let backdrop = backdrop_node(&look);
    let color = socket(GraphRef::World, backdrop.get(), "color");
    look.keys.apply(
        look.keys
            .inserting(key(&color, 4, LookValue::Vec3([1.0, 0.0, 0.0]))),
    );
    let before = look.clone();
    let undo = look
        .apply(
            library,
            LookEdit::Graph {
                graph: GraphRef::World,
                edit: Edit::RemoveNode {
                    scope: Scope::Main,
                    id: backdrop,
                },
            },
        )
        .expect("removes the node");
    assert!(look.keys.is_empty());
    look.apply(library, undo).expect("restores it");
    assert_eq!(look, before);
}

#[test]
fn material_renames_and_removals_carry_slots_and_tracks() {
    let library = library();
    let mut look = Look::default();
    let material = Arc::new(rikka_shader_graph::Graph::new(GraphKind::Material));
    look.apply(
        library,
        LookEdit::InsertMaterial {
            key: "skin".into(),
            graph: material,
        },
    )
    .expect("inserts");
    let miku = Name::new("miku");
    look.slots.assign(miku, 2, Some("skin".into()));
    let target = socket(GraphRef::Material("skin".into()), 0, "base");
    look.keys
        .apply(look.keys.inserting(key(&target, 0, LookValue::Float(0.5))));

    let renamed = look
        .apply(
            library,
            LookEdit::RenameMaterial {
                from: "skin".into(),
                to: "face".into(),
            },
        )
        .expect("renames");
    assert_eq!(
        look.slots
            .slot(miku, 2)
            .and_then(|slot| slot.material.as_deref()),
        Some("face")
    );
    assert_eq!(
        look.keys.tracks().keyed().cloned().collect::<Vec<_>>(),
        [socket(GraphRef::Material("face".into()), 0, "base")]
    );
    look.apply(library, renamed).expect("renames back");

    let before = look.clone();
    let undo = look
        .apply(library, LookEdit::RemoveMaterial { key: "skin".into() })
        .expect("removes");
    assert!(look.materials.is_empty() && look.keys.is_empty());
    assert_eq!(look.slots.slot(miku, 2), None);
    look.apply(library, undo).expect("restores");
    assert_eq!(look, before);
}

#[test]
fn a_world_light_keyed_in_place() {
    let graph = wired(&[("world.light", vec![("intensity", 2.0.into())])]);
    let mut look = Look {
        world: Arc::new(graph),
        ..Look::default()
    };
    let light = look
        .world
        .body()
        .nodes()
        .iter()
        .find(|(_, node)| node.kind == NodeKind::Extern("world.light".into()))
        .map(|(id, _)| *id)
        .expect("a light");
    let intensity = socket(GraphRef::World, light.get(), "intensity");
    look.keys.apply(
        look.keys
            .inserting(key(&intensity, 0, LookValue::Float(0.0))),
    );
    look.keys.apply(
        look.keys
            .inserting(key(&intensity, 10, LookValue::Float(8.0))),
    );
    assert_eq!(evaluated_world(&look, 2.5).lights[0].intensity, 2.0);
}
