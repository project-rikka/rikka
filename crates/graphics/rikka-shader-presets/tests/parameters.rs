use rikka_shader_graph::{Graph, Library, NodeKind};
use rikka_shader_presets::{import, presets};
use rikka_slang::{ParameterKind, Role, TextureKind};

fn color(alpha: bool) -> impl Fn(&ParameterKind) -> bool {
    move |kind| matches!(kind, ParameterKind::Color { alpha: found, .. } if *found == alpha)
}

fn texture(wanted: TextureKind) -> impl Fn(&ParameterKind) -> bool {
    move |kind| matches!(kind, ParameterKind::Texture { kind: found, sampler: true } if *found == wanted)
}

fn float(kind: &ParameterKind) -> bool {
    *kind == ParameterKind::Float
}

type Kind = Box<dyn Fn(&ParameterKind) -> bool>;

fn imports() -> Vec<(&'static str, Kind, Option<Role>)> {
    vec![
        (
            import::BASE_COLOR,
            Box::new(color(true)),
            Some(Role::Albedo),
        ),
        (
            import::BASE_TEXTURE,
            Box::new(texture(TextureKind::Colour)),
            Some(Role::AlbedoTexture),
        ),
        (import::SHADE_COLOR, Box::new(color(false)), None),
        (
            import::EMISSION_COLOR,
            Box::new(color(false)),
            Some(Role::Emission),
        ),
        (
            import::EMISSION_TEXTURE,
            Box::new(texture(TextureKind::Colour)),
            Some(Role::EmissionTexture),
        ),
        (
            import::NORMAL_TEXTURE,
            Box::new(texture(TextureKind::Normal)),
            None,
        ),
        (import::NORMAL_SCALE, Box::new(float), None),
        (import::OUTLINE_WIDTH, Box::new(float), None),
        (import::OUTLINE_COLOR, Box::new(color(false)), None),
    ]
}

fn built(library: &Library) -> Vec<(&'static str, Graph)> {
    presets()
        .into_iter()
        .map(|preset| {
            let graph = preset
                .graph(library)
                .unwrap_or_else(|error| panic!("{}: {error}", preset.key));
            (preset.key, graph)
        })
        .collect()
}

#[test]
fn presets_declare_the_shared_import_parameters_alike() {
    let library = Library::standard();
    for (key, graph) in built(&library) {
        for required in [import::BASE_COLOR, import::BASE_TEXTURE] {
            assert!(
                graph.parameter(required).is_some(),
                "{key} lacks {required}"
            );
        }
        for (name, fits, role) in imports() {
            let Some(parameter) = graph.parameter(name) else {
                continue;
            };
            assert!(
                fits(&parameter.kind),
                "{key}.{name} is a {:?}",
                parameter.kind
            );
            assert_eq!(parameter.role, role, "{key}.{name}");
        }
    }
}

#[test]
fn every_declared_parameter_feeds_the_graph() {
    let library = Library::standard();
    for (key, graph) in built(&library) {
        for parameter in graph.parameters() {
            let used = graph.scopes().any(|(_, body)| {
                body.nodes().iter().any(|(id, node)| {
                    node.kind == NodeKind::Parameter(parameter.name.clone())
                        && body
                            .connections()
                            .iter()
                            .any(|connection| connection.from.node == *id)
                })
            });
            assert!(used, "{key} declares {} without using it", parameter.name);
        }
    }
}
