use std::sync::Arc;

use animate_look::SocketKey;
use animate_shader::{Job, Location, Shaders, Status};
use rikka_renderer_3d::ShaderCompiler;
use rikka_shader_graph::{Constant, NodeKind, PortRef, Scope, SocketRef};

use crate::support::{Bench, find, skin};

fn principled(bench: &Bench) -> rikka_shader_graph::NodeId {
    find(
        &bench.look,
        &skin(),
        |kind| matches!(kind, NodeKind::Extern(id) if id == "bsdf.principled"),
    )
}

fn request(shaders: &mut Shaders, bench: &Bench) -> Option<Job> {
    let at = skin();
    shaders.request(
        "Skin",
        bench.workbench.graph_revision(&at).unwrap(),
        bench.look.graph(&at).unwrap(),
        bench.workbench.library(),
    )
}

fn roughness(bench: &Bench) -> SocketKey {
    SocketKey::new(
        skin(),
        SocketRef::new(Scope::Main, principled(bench), "roughness"),
    )
}

fn set(bench: &mut Bench, key: &SocketKey, value: f32) {
    let edit = bench.workbench.set_socket(key, Some(value.into()));
    bench.apply(edit);
}

#[test]
fn only_source_changes_ask_for_a_compile() {
    let compiler = ShaderCompiler::new().expect("libslang loads");
    let mut bench = Bench::new();
    let mut shaders = Shaders::new();
    let key = roughness(&bench);
    set(&mut bench, &key, 0.4);
    let job = request(&mut shaders, &bench).expect("a first compile");
    assert_eq!(shaders.status("Skin"), Some(Status::Compiling));
    assert!(request(&mut shaders, &bench).is_none(), "the same revision");
    let outcome = job.run(&compiler);
    assert!(outcome.result.is_ok(), "{:?}", outcome.result.err());
    assert!(shaders.finish(outcome).is_none());
    assert_eq!(shaders.status("Skin"), Some(Status::Ready));
    let live = shaders.material("Skin").expect("a compiled material");
    assert!(live.sockets().iter().any(|socket| socket.at == key.socket));

    for value in [0.6, 0.8] {
        set(&mut bench, &key, value);
        assert!(
            request(&mut shaders, &bench).is_none(),
            "a set socket is a live uniform"
        );
    }
    assert_eq!(shaders.status("Skin"), Some(Status::Ready));

    let link = |bench: &mut Bench, port: &'static str| {
        let ((), edit) = bench
            .workbench
            .change(&bench.look, &skin(), |graph, library| {
                let mut editor = graph.edit(library, Scope::Main);
                let constant =
                    editor.add_node(NodeKind::Constant(Constant::scalar(0.1)), [0.0, 0.0])?;
                editor.connect(
                    PortRef::new(constant.value, "value"),
                    PortRef::new(key.socket.node, port),
                )
            })
            .unwrap();
        bench.apply(edit);
    };
    link(&mut bench, "metalness");
    let rebuild = request(&mut shaders, &bench).expect("a new link recompiles");
    link(&mut bench, "specular_weight");
    assert!(
        request(&mut shaders, &bench).is_none(),
        "a newer source waits for the running job"
    );
    assert_eq!(shaders.status("Skin"), Some(Status::Compiling));
    assert!(Arc::ptr_eq(&shaders.material("Skin").unwrap(), &live));
    assert_eq!(
        (rebuild.key.as_str(), rebuild.revision),
        ("Skin", bench.workbench.graph_revision(&skin()).unwrap() - 1)
    );
}

#[test]
fn an_incomplete_graph_fails_and_keeps_the_last_good_material() {
    let compiler = ShaderCompiler::new().expect("libslang loads");
    let mut bench = Bench::new();
    let mut shaders = Shaders::new();
    let job = request(&mut shaders, &bench).unwrap();
    assert!(shaders.finish(job.run(&compiler)).is_none());
    let good = shaders.material("Skin").unwrap();

    let output = find(&bench.look, &skin(), |kind| *kind == NodeKind::Output);
    let ((), edit) = bench
        .workbench
        .change(&bench.look, &skin(), |graph, library| {
            graph.edit(library, Scope::Main).remove_node(output)
        })
        .unwrap();
    let undo = bench.apply(edit);
    assert!(
        request(&mut shaders, &bench).is_none(),
        "nothing to compile"
    );
    let Some(Status::Failed(diagnostics)) = shaders.status("Skin") else {
        panic!(
            "the graph without output fails: {:?}",
            shaders.status("Skin")
        );
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.at == Location::Graph),
        "{diagnostics:?}"
    );
    assert!(Arc::ptr_eq(&shaders.material("Skin").unwrap(), &good));
    assert_eq!(&*shaders.diagnostics("Skin"), &*diagnostics);

    bench.apply(undo);
    assert!(
        request(&mut shaders, &bench).is_none(),
        "the built source again"
    );
    assert_eq!(shaders.status("Skin"), Some(Status::Ready));
}
