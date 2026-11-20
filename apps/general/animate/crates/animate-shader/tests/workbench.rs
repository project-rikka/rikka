use animate_look::{GraphRef, LookEdit, SocketKey};
use animate_shader::{Synced, empty_material};
use rikka_mmd::VmdCurve;
use rikka_shader_graph::{NodeKind, Scope, SocketRef};

use crate::support::{Bench, find, library, skin};

fn roughness(bench: &Bench) -> SocketKey {
    let node = find(
        &bench.look,
        &skin(),
        |kind| matches!(kind, NodeKind::Extern(id) if id == "bsdf.principled"),
    );
    SocketKey::new(skin(), SocketRef::new(Scope::Main, node, "roughness"))
}

#[test]
fn sync_names_changed_renamed_and_removed_graphs() {
    let mut bench = Bench::new();
    let start = bench.workbench.revision();
    let edit = bench
        .workbench
        .set_socket(&roughness(&bench), Some(0.3.into()));
    let library = library();
    bench.look.apply(&library, edit).unwrap();
    assert_eq!(bench.workbench.sync(&bench.look), [Synced::Changed(skin())]);
    assert!(bench.workbench.revision() > start);
    assert!(bench.workbench.sync(&bench.look).is_empty());

    bench
        .look
        .apply(
            &library,
            LookEdit::RenameMaterial {
                from: "Skin".to_owned(),
                to: "Face".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(
        bench.workbench.sync(&bench.look),
        [Synced::Renamed {
            from: "Skin".to_owned(),
            to: "Face".to_owned(),
        }]
    );

    bench
        .look
        .apply(
            &library,
            LookEdit::RemoveMaterial {
                key: "Face".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(
        bench.workbench.sync(&bench.look),
        [Synced::Removed("Face".to_owned())]
    );
    assert_eq!(bench.workbench.graph_revision(&skin()), None);
}

#[test]
fn graph_changes_drop_tracks_of_vanished_sockets_and_undo_restores_them() {
    let mut bench = Bench::new();
    let key = roughness(&bench);
    let library = library();
    bench
        .look
        .key_socket(&library, key.clone(), 10, 0.2.into(), VmdCurve::LINEAR)
        .unwrap();
    assert!(!bench.look.keys.is_empty());

    let node = key.socket.node;
    let ((), edit) = bench
        .workbench
        .change(&bench.look, &skin(), |graph, library| {
            graph.edit(library, Scope::Main).remove_node(node)
        })
        .unwrap();
    let undo = bench.apply(edit);
    assert!(bench.look.keys.is_empty());
    assert!(
        bench
            .look
            .graph(&skin())
            .unwrap()
            .body()
            .node(node)
            .is_none()
    );

    bench.apply(undo);
    assert!(!bench.look.keys.is_empty());
    assert!(
        bench
            .look
            .graph(&skin())
            .unwrap()
            .body()
            .node(node)
            .is_some()
    );
}

#[test]
fn added_materials_take_fresh_valid_names() {
    let mut bench = Bench::new();
    let library = library();
    let (copy, edit) = bench
        .workbench
        .duplicate_material(&bench.look, "Skin")
        .unwrap();
    assert_eq!(copy, "Skin 2");
    bench.apply(edit);
    let (hair, edit) = bench
        .workbench
        .add_material(&bench.look, "Hair", empty_material(&library).unwrap())
        .unwrap();
    assert_eq!(hair, "Hair");
    bench.apply(edit);
    assert!(
        bench
            .look
            .apply(
                &library,
                LookEdit::RenameMaterial {
                    from: "Hair".to_owned(),
                    to: " bad".to_owned(),
                },
            )
            .is_err()
    );
    let world = bench.look.world.clone();
    assert!(
        bench
            .workbench
            .add_material(
                &bench.look,
                "World",
                rikka_shader_graph::Graph::clone(&world)
            )
            .is_err(),
        "a world graph is no material"
    );
    assert_eq!(
        bench.look.graphs().map(|(at, _)| at).collect::<Vec<_>>(),
        [
            GraphRef::World,
            GraphRef::Composite,
            GraphRef::Material("Hair".to_owned()),
            skin(),
            GraphRef::Material("Skin 2".to_owned()),
        ]
    );
}
