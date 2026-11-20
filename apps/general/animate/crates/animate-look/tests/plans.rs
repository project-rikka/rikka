use std::sync::Arc;

use animate_look::{GraphRef, Look, LookEdit, Plans};
use rikka_shader_graph::{Datum, Edit, NodeKind, Scope};

use crate::support::library;

fn exposure(look: &Look) -> rikka_shader_graph::NodeId {
    look.composite
        .body()
        .nodes()
        .iter()
        .find(|(_, node)| node.kind == NodeKind::Extern("filter.exposure".into()))
        .map(|(id, _)| *id)
        .expect("the default composite exposes")
}

#[test]
fn plans_are_kept_until_their_graph_changes() {
    let library = library();
    let mut look = Look::default();
    let mut plans = Plans::default();
    let first = Arc::clone(
        &plans
            .evaluate(&look, library, 0.0, 0.0)
            .expect("evaluates")
            .composite,
    );
    let again = Arc::clone(
        &plans
            .evaluate(&look, library, 9.0, 0.3)
            .expect("evaluates")
            .composite,
    );
    assert!(
        Arc::ptr_eq(&first, &again),
        "an unchanged graph is not planned again"
    );

    let node = exposure(&look);
    look.apply(
        library,
        LookEdit::Graph {
            graph: GraphRef::Composite,
            edit: Edit::SetInput {
                scope: Scope::Main,
                id: node,
                port: "compensation".into(),
                value: Some(1.5.into()),
            },
        },
    )
    .expect("sets the exposure");
    let evaluated = plans.evaluate(&look, library, 0.0, 0.0).expect("evaluates");
    assert!(!Arc::ptr_eq(&first, &evaluated.composite));
    let compensation = evaluated
        .records
        .iter()
        .flatten()
        .find(|record| record.id == "filter.exposure")
        .and_then(|record| record.float("compensation"));
    assert_eq!(compensation, Some(1.5));

    let kept = Arc::clone(&evaluated.composite);
    let mut world = look.clone();
    world.world = Arc::new(rikka_shader_graph::Graph::clone(&look.world));
    let changed = plans
        .evaluate(&world, library, 0.0, 0.0)
        .expect("evaluates");
    assert!(
        Arc::ptr_eq(&kept, &changed.composite),
        "a world change keeps the composite plan"
    );
    assert!(matches!(
        changed.records[0]
            .as_ref()
            .and_then(|record| record.field("compensation")),
        Some(Datum::Number(_))
    ));
}

#[test]
fn material_values_follow_sockets_slots_and_slot_keys() {
    use animate_look::{LookKey, LookValue, MaterialValue, Name, SlotKey, Target};
    use rikka_mmd::VmdCurve;
    use rikka_renderer_3d::Value;
    use rikka_shader_graph::{Builder, GraphKind, ParameterDecl, SocketRef};

    let library = library();
    let mut builder = Builder::new(GraphKind::Material, library);
    let bsdf = builder.external("bsdf.principled", [("roughness", 0.3.into())]);
    let shader = builder.out(bsdf, "shader");
    builder.output([("surface", shader.into())]);
    let mut graph = builder.finish().expect("a valid material");
    graph
        .apply(
            library,
            Edit::InsertParameter {
                at: 0,
                parameter: ParameterDecl::float("glow", 0.0),
            },
        )
        .expect("the parameter inserts");
    let sockets = graph.material(library, "skin").expect("generates").sockets;
    let roughness = sockets
        .iter()
        .find(|socket| socket.at == SocketRef::new(Scope::Main, bsdf, "roughness"))
        .expect("a set socket is a uniform")
        .name
        .clone();

    let miku = Name::new("miku");
    let glow = Name::new("glow");
    let mut look = Look::default();
    look.apply(
        library,
        LookEdit::InsertMaterial {
            key: "Skin".into(),
            graph: Arc::new(graph),
        },
    )
    .unwrap();
    look.slots.assign(miku, 1, Some("Skin".into()));
    look.slots.set(miku, 1, glow, LookValue::Float(0.25));
    look.slots
        .set(miku, 1, Name::new("unknown"), LookValue::Float(9.0));
    let target = Target::Slot(SlotKey {
        figure: miku,
        material: 1,
        parameter: glow,
    });
    for (frame, value) in [(10, 0.0), (20, 1.0)] {
        let inserting = look.keys.inserting(LookKey {
            target: target.clone(),
            frame,
            value: LookValue::Float(value),
            curve: VmdCurve::LINEAR,
        });
        look.keys.apply(inserting);
    }

    let mut plans = Plans::default();
    let at = |frame: f32, plans: &mut Plans| {
        plans
            .evaluate(&look, library, frame, 0.0)
            .unwrap()
            .material_values(miku, 1, "Skin", &sockets)
    };
    let values = at(15.0, &mut plans);
    assert!(values.contains(&(roughness.clone(), MaterialValue::Value(Value::Float(0.3)))));
    assert!(values.contains(&("glow".into(), MaterialValue::Value(Value::Float(0.5)))));
    assert!(!values.iter().any(|(name, _)| name == "unknown"));
    let resting = plans
        .evaluate(&look, library, 15.0, 0.0)
        .unwrap()
        .material_values(miku, 2, "Skin", &sockets);
    assert!(!resting.iter().any(|(name, _)| name == "glow"));
}
