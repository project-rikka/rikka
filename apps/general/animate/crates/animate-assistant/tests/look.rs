use animate_assistant::{
    Assigned, Assignment, FigureSlots, Given, GraphEdits, GraphOp, KeySet, KeyTarget, LookKeys,
    LookReset, MaterialEdits, NodeRef, Plug, SlotInfo, SlotPick, SlotRef, SlotValue, StarterKind,
    apply_graph_edits, apply_keys, apply_materials, reset_look,
};
use animate_look::{GraphRef, Look, Name, shared_library};
use rikka_mmd::VmdCurve;
use rikka_shader_graph::{Edit, NodeKind, ParameterDecl};

fn output(look: &Look) -> NodeRef {
    let id = look
        .world
        .body()
        .nodes()
        .iter()
        .find(|(_, node)| node.kind == NodeKind::Output)
        .map(|(id, _)| *id)
        .expect("the world has an output");
    NodeRef::Id(id)
}

fn plug(node: NodeRef, port: &str) -> Plug {
    Plug {
        node,
        port: port.to_owned(),
    }
}

fn miku() -> FigureSlots {
    FigureSlots {
        handle: "miku".into(),
        name: Name::new("miku"),
        materials: ["body", "hair", "bangs"]
            .into_iter()
            .zip([Some("skin"), Some("hair"), Some("hair")])
            .map(|(name, class)| SlotInfo {
                name: name.into(),
                class,
            })
            .collect(),
    }
}

#[test]
fn graph_edits_apply_whole_or_not_at_all() {
    let library = shared_library();
    let mut look = Look::default();
    let key = NodeRef::Named("key".into());
    let world_output = output(&look);
    let added = apply_graph_edits(
        &mut look,
        library,
        &GraphEdits {
            graph: GraphRef::World,
            ops: vec![
                GraphOp::Add {
                    name: Some("key".into()),
                    kind: NodeKind::Extern("world.light".into()),
                    at: None,
                    values: vec![("azimuth".into(), Given::Numbers(vec![90.0]))],
                },
                GraphOp::Connect {
                    from: plug(key.clone(), "light"),
                    to: plug(world_output, "lights"),
                },
            ],
        },
    )
    .expect("the light wires in");
    let (_, id) = added[0];
    let azimuth = look
        .world
        .body()
        .node(id)
        .and_then(|node| node.inputs.get("azimuth"))
        .and_then(|value| value.number())
        .expect("the azimuth is set");
    assert!(
        (azimuth.component(0) - 90f32.to_radians()).abs() < 1e-5,
        "degrees become radians"
    );

    let before = look.clone();
    let refused = apply_graph_edits(
        &mut look,
        library,
        &GraphEdits {
            graph: GraphRef::World,
            ops: vec![
                GraphOp::Set {
                    to: plug(NodeRef::Id(id), "intensity"),
                    value: Given::Numbers(vec![2.0]),
                },
                GraphOp::Set {
                    to: plug(NodeRef::Id(id), "wattage"),
                    value: Given::Numbers(vec![1.0]),
                },
            ],
        },
    )
    .expect_err("there is no wattage");
    assert!(refused.to_string().starts_with("edit 1"), "{refused}");
    assert_eq!(look, before);
}

#[test]
fn materials_assign_by_class_and_take_slot_values_and_keys() {
    let library = shared_library();
    let mut look = Look::default();
    let figures = [miku()];
    apply_materials(
        &mut look,
        library,
        &figures,
        &MaterialEdits {
            add: vec![("Hair".into(), None)],
            assign: vec![Assignment {
                figure: "miku".into(),
                slots: SlotPick::Class("hair".into()),
                material: Assigned::Graph("Hair".into()),
                unique: false,
            }],
            ..MaterialEdits::default()
        },
        0,
        VmdCurve::LINEAR,
    )
    .expect("the hair graph is assigned");
    let assigned: Vec<u32> = look
        .slots
        .figure(Name::new("miku"))
        .filter(|(_, slot)| slot.material.as_deref() == Some("Hair"))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(assigned, [1, 2]);

    let graph = std::sync::Arc::make_mut(look.materials.get_mut("Hair").expect("added"));
    graph
        .apply(
            library,
            Edit::InsertParameter {
                at: 0,
                parameter: ParameterDecl::float("sheen", 0.0).with_range(0.0, 1.0),
            },
        )
        .expect("a parameter");
    let value = |value: f32| SlotValue {
        figure: "miku".into(),
        slot: SlotRef::Named("bangs".into()),
        parameter: "sheen".into(),
        value: Some(Given::Numbers(vec![value])),
    };
    let edits = |value| MaterialEdits {
        values: vec![value],
        ..MaterialEdits::default()
    };
    assert!(
        apply_materials(
            &mut look,
            library,
            &figures,
            &edits(value(3.0)),
            0,
            VmdCurve::LINEAR
        )
        .is_err()
    );
    apply_materials(
        &mut look,
        library,
        &figures,
        &edits(value(0.5)),
        0,
        VmdCurve::LINEAR,
    )
    .expect("in range");

    apply_keys(
        &mut look,
        library,
        &figures,
        &LookKeys {
            set: vec![KeySet {
                target: KeyTarget::Slot {
                    figure: "miku".into(),
                    slot: SlotRef::Index(2),
                    parameter: "sheen".into(),
                },
                frame: 30,
                value: None,
                curve: None,
            }],
            remove: Vec::new(),
        },
        VmdCurve::LINEAR,
    )
    .expect("the current value is keyed");
    assert_eq!(look.keys.tracks().all_keys().count(), 1);

    apply_materials(
        &mut look,
        library,
        &figures,
        &MaterialEdits {
            remove: vec!["Hair".into()],
            ..MaterialEdits::default()
        },
        0,
        VmdCurve::LINEAR,
    )
    .expect("removed");
    assert!(
        look.slots
            .figure(Name::new("miku"))
            .all(|(_, slot)| slot.material.is_none())
    );
}

#[test]
fn starters_replace_world_and_composite() {
    let library = shared_library();
    let mut look = Look::default();
    let bare = look.world.clone();
    reset_look(
        &mut look,
        library,
        LookReset {
            world: Some(StarterKind::Cinematic),
            composite: None,
        },
    )
    .expect("the cinematic world builds");
    assert_ne!(look.world, bare);
}
