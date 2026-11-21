use animate_look::world::{
    AntialiasingMode, Backdrop, FloorKind, LightKind, LightUnit, OverrideValue, StageLight,
};
use animate_look::{
    Linked, Look, Name, World, cinematic_composite, cinematic_world, default_composite, world_nodes,
};
use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    Antialiasing, Background, Composite, Environment, Literal, Metering, Operation, ParameterKind,
    Resampling, ShadowQuality, Tone, Value, ViewportConfig,
};
use rikka_shader_graph::{Hint, PortDefault, PortSpec, PortType, SocketValue, ValueType};

use crate::support::{Lookup, evaluated_world, library, spec, wired, world_of};

fn varied(port: &PortSpec, held: Option<&SocketValue>) -> SocketValue {
    if port.ty == PortType::Value(ValueType::Texture) {
        return match (port.hint, held) {
            (Hint::Text, Some(_)) => "night".into(),
            (Hint::Text, None) => "miku".into(),
            _ => "assets/file.ies".into(),
        };
    }
    let PortDefault::Value(default) = port.default else {
        panic!("{} has a value default", port.name);
    };
    let first = default.component(0);
    match port.hint {
        Hint::Toggle => (1.0 - first).into(),
        Hint::Options(options) => ((first + 1.0) % options.len() as f32).into(),
        Hint::Integer => (first + 1.0).into(),
        _ => {
            let shifted: Vec<f32> = default.values().iter().map(|value| value + 0.25).collect();
            SocketValue::Number(
                rikka_shader_graph::Constant::from_slice(&shifted).expect("a value"),
            )
        }
    }
}

#[test]
fn every_world_socket_reaches_the_typed_world() {
    for spec in world_nodes() {
        let baseline: Vec<(&str, SocketValue)> = match spec.id {
            "world.override" => vec![("parameter", "day".into())],
            _ => Vec::new(),
        };
        let before = world_of(&wired(&[(spec.id, baseline.clone())]));
        for port in &spec.inputs {
            let held = baseline
                .iter()
                .find(|(name, _)| *name == port.name)
                .map(|(_, value)| value);
            let mut sockets: Vec<(&str, SocketValue)> = baseline
                .iter()
                .filter(|(name, _)| *name != port.name)
                .cloned()
                .collect();
            sockets.push((port.name, varied(port, held)));
            let after = world_of(&wired(&[(spec.id, sockets)]));
            assert_ne!(before, after, "{} {} is lost", spec.id, port.name);
        }
    }
}

#[test]
fn sockets_arrive_in_their_units() {
    type Check = fn(&World) -> bool;
    let cases: [(&str, &str, SocketValue, Check); 10] = [
        ("world.light", "kind", 2.0.into(), |world| {
            world.lights[0].kind == LightKind::Spot
        }),
        ("world.light", "unit", 2.0.into(), |world| {
            world.lights[0].unit == LightUnit::Candela
        }),
        ("world.light", "azimuth", 1.25.into(), |world| {
            world.lights[0].azimuth == 1.25
        }),
        ("world.light", "figures", " miku, rin ,".into(), |world| {
            world.lights[0].figures == [Name::new("miku"), Name::new("rin")]
        }),
        ("world.shadows", "samples", 2.6.into(), |world| {
            world.shadows.samples == 3
        }),
        ("world.sampling", "antialiasing", 3.0.into(), |world| {
            world.sampling.antialiasing == AntialiasingMode::Dynamic
        }),
        (
            "world.sky",
            "shadows",
            1.0.into(),
            |world| matches!(world.backdrop, Some(Backdrop::Sky(sky)) if sky.shadows == ShadowQuality::Low),
        ),
        ("world.floor", "kind", 4.0.into(), |world| {
            world
                .floor
                .is_some_and(|floor| floor.kind == FloorKind::Water)
        }),
        ("world.illumination", "resampling", 0.0.into(), |world| {
            world
                .illumination
                .is_some_and(|illumination| illumination.resampling == Resampling::Off)
        }),
        (
            "world.panorama",
            "image",
            "skies/dusk.hdr".into(),
            |world| {
                matches!(&world.backdrop, Some(Backdrop::Panorama(panorama))
                if panorama.image.as_deref() == Some("skies/dusk.hdr"))
            },
        ),
    ];
    for (node, port, value, check) in cases {
        let world = world_of(&wired(&[(node, vec![(port, value)])]));
        assert!(check(&world), "{node} {port}: {world:?}");
    }
}

#[test]
fn default_look_is_the_bare_mmd_stage() {
    let world = evaluated_world(&Look::default(), 0.0);
    assert_eq!(world.backdrop, Some(Backdrop::Color(Vec3::ONE)));
    assert_eq!(world.stage_light, Some(StageLight::default()));
    assert_eq!(world.shadows.screen.strength, 0.0);
    assert_eq!(
        world
            .drawn_figure("miku", None, &mut Lookup::default())
            .shadow,
        ShadowQuality::High,
        "every figure casts through its own map"
    );
    assert_eq!(world.floor.map(|floor| floor.kind), Some(FloorKind::Shadow));
    assert!(world.grid());
    assert_eq!(world.sampling.antialiasing, AntialiasingMode::Temporal);
    let look = world.look(&mut Lookup::default());
    assert_eq!(look.environment, Environment::None);
    assert_eq!(look.fog.density, 0.0);
    assert_eq!(look.occlusion.strength, 0.0);
    assert_eq!(world.background(), Background::Color(Vec3::ONE));
    let sun = world.sunlight().expect("the stage light leads");
    assert!(sun.toward.y < 0.0);
    assert!(
        world
            .ambient()
            .abs_diff_eq(Vec3::splat(154.0 / 255.0 / 6.0), 1e-6)
    );
    let config = world.configure(ViewportConfig::default());
    assert_eq!(config.antialiasing, Antialiasing::Temporal);
}

#[test]
fn unwired_optional_features_are_off() {
    let world = world_of(&wired(&[]));
    assert_eq!(world, World::default());
    let look = world.look(&mut Lookup::default());
    assert_eq!(look.fog.density, 0.0);
    assert_eq!(look.occlusion.strength, 0.0);
    assert_eq!(look.reflections.strength, 0.0);
    assert_eq!(look.illumination.strength, 0.0);
    assert_eq!(look.lines.strength, 0.0);
    assert_eq!(look.particles.amount, 0.0);
    assert!(look.emitters.is_empty());
    assert!(world.floor().is_none() && !world.grid());
    assert_eq!(world.sunlight(), None);
    assert_eq!(world.ambient(), Vec3::ZERO);
    assert_eq!(world.background(), Background::Color(Vec3::ZERO));
}

#[test]
fn sky_takes_over_the_lead_from_the_stage_light() {
    let world = world_of(&wired(&[
        ("world.sky", vec![("hour", 18.5.into())]),
        ("world.stage_light", Vec::new()),
    ]));
    let look = world.look(&mut Lookup::default());
    assert!(matches!(look.environment, Environment::Sky(sky) if sky.hour == 18.5));
    assert_eq!(world.sunlight(), None);
    assert_ne!(world.ambient(), Vec3::ZERO);
    assert_eq!(world.background(), Background::Sky);
}

#[test]
fn panorama_waits_for_its_image() {
    let world = world_of(&wired(&[(
        "world.panorama",
        vec![("image", "skies/dusk.hdr".into()), ("sun", 1.0.into())],
    )]));
    let mut lookup = Lookup::default();
    assert_eq!(world.look(&mut lookup).environment, Environment::None);
    assert_eq!(lookup.panoramas, ["skies/dusk.hdr"]);
    assert!(
        world
            .lights(Vec3::ZERO, Vec3::Z, &[], &mut lookup)
            .is_empty()
    );
    assert_eq!(
        world
            .assets()
            .map(|asset| asset.as_str())
            .collect::<Vec<_>>(),
        ["skies/dusk.hdr"]
    );
}

#[test]
fn lights_place_around_the_focus_in_their_units() {
    let world = world_of(&wired(&[(
        "world.light",
        vec![
            ("kind", 0.0.into()),
            ("unit", 1.0.into()),
            ("intensity", 64_000.0.into()),
            ("azimuth", 0.0.into()),
            ("elevation", 0.0.into()),
            ("anchor", 1.0.into()),
        ],
    )]));
    let lights = world.lights(
        Vec3::ZERO,
        Vec3::new(0.0, 1.0, 5.0),
        &[],
        &mut Lookup::default(),
    );
    let drawn = lights[0]
        .as_ref()
        .expect("lux measures a directional light");
    let reference = rikka_renderer_3d::Light::directional(Vec3::NEG_Z)
        .lux(64_000.0)
        .expect("lux");
    assert!((drawn.intensity - reference.intensity).abs() < 1e-6);
    assert!(matches!(
        drawn.kind,
        rikka_renderer_3d::LightKind::Directional { direction } if direction.abs_diff_eq(Vec3::NEG_Z, 1e-5)
    ));

    let point = world_of(&wired(&[(
        "world.light",
        vec![("unit", 1.0.into()), ("intensity", 3.0.into())],
    )]));
    assert_eq!(point.lights[0].measured(), LightUnit::Relative);
    let lit = point.lights(Vec3::ZERO, Vec3::Z, &[], &mut Lookup::default());
    assert_eq!(lit[0].as_ref().map(|light| light.intensity).ok(), Some(3.0));

    let camera = world_of(&wired(&[(
        "world.light",
        vec![("azimuth", 0.0.into()), ("elevation", 0.0.into())],
    )]));
    let offset = camera.lights[0].offset(Vec3::ZERO, Vec3::new(5.0, 2.0, 0.0));
    assert!(
        offset.abs_diff_eq(Vec3::X, 1e-5),
        "camera anchored lights face the eye: {offset}"
    );
}

#[test]
fn linked_lights_shine_on_their_figures_only() {
    let world = world_of(&wired(&[
        ("world.light", vec![("figures", "miku".into())]),
        ("world.light", vec![("enabled", 0.0.into())]),
    ]));
    let figures = [
        Linked {
            name: "miku",
            channel: 1 << 2,
        },
        Linked {
            name: "rin",
            channel: 1 << 3,
        },
    ];
    let lights = world.lights(Vec3::ZERO, Vec3::Z, &figures, &mut Lookup::default());
    assert_eq!(lights.len(), 1, "a disabled light does not shine");
    assert_eq!(
        lights[0].as_ref().map(|light| light.channels).ok(),
        Some(1 << 2)
    );
}

#[test]
fn overrides_and_rigs_follow_their_figures() {
    let world = world_of(&wired(&[
        (
            "world.override",
            vec![("parameter", "day".into()), ("value", 0.25.into())],
        ),
        (
            "world.override",
            vec![
                ("parameter", "rim".into()),
                ("figure", "miku".into()),
                ("value", 4.0.into()),
            ],
        ),
        (
            "world.override",
            vec![
                ("parameter", "ramp".into()),
                ("figure", "miku".into()),
                ("texture", "ramps/warm.png".into()),
            ],
        ),
        ("world.override", vec![("parameter", "unknown".into())]),
        (
            "world.rig",
            vec![
                ("figure", "miku".into()),
                ("turns", 0.0.into()),
                ("shadow", 3.0.into()),
            ],
        ),
    ]));
    assert!(matches!(
        &world.overrides[2].value,
        OverrideValue::Texture(path) if path.as_str() == "ramps/warm.png"
    ));
    let mut lookup = Lookup::default();
    lookup.parameters.insert(
        "day".into(),
        spec("day", ParameterKind::Float, Literal::Float(1.0)),
    );
    lookup.parameters.insert(
        "rim".into(),
        spec("rim", ParameterKind::Float, Literal::Float(0.5)),
    );
    lookup.parameters.insert(
        "ramp".into(),
        spec(
            "ramp",
            ParameterKind::Texture {
                kind: rikka_renderer_3d::TextureKind::Ramp,
                sampler: true,
            },
            Literal::Texture,
        ),
    );
    let look = world.look(&mut lookup);
    assert_eq!(look.overrides.get("day"), Some(&Value::Float(0.25)));
    assert_eq!(look.overrides.get("rim"), None);
    let miku = world.drawn_figure("miku", None, &mut lookup);
    assert_eq!(
        miku.overrides.get("rim"),
        Some(&Value::Float(1.0)),
        "clamped to the range"
    );
    assert_eq!(miku.overrides.get("ramp"), Some(&Value::Texture(None)));
    assert_eq!(
        lookup.textures,
        [(
            "ramps/warm.png".to_owned(),
            rikka_renderer_3d::TextureKind::Ramp
        )]
    );
    let key = miku.rig.key.expect("a key light");
    assert_eq!(key.direction, None);
    assert_eq!(key.intensity, Some(3.0));
    assert_eq!(miku.shadow, ShadowQuality::High);
    let rin = world.drawn_figure("rin", None, &mut lookup);
    assert_eq!(rin.rig.key, None);
    assert!(rin.overrides.get("rim").is_none());
}

#[test]
fn default_composite_is_bare_mmd() {
    let graph = default_composite(library()).expect("a composite");
    let composite = bridged(&graph);
    let operations: Vec<&Operation> = composite.nodes.iter().map(|node| &node.operation).collect();
    assert!(matches!(
        operations[..],
        [Operation::Exposure(exposure), Operation::Tonemap(tonemap)]
            if exposure.metering == Metering::Manual
                && exposure.compensation == 0.0
                && tonemap.tone == Tone::Clamp
    ));
}

#[test]
fn cinematic_graphs_match_the_renderer_defaults() {
    let world = world_of(&cinematic_world(library()).expect("a world"));
    let expected = rikka_renderer_3d::Look {
        composite: Composite::new(),
        ..rikka_renderer_3d::Look::default()
    };
    assert_eq!(world.look(&mut Lookup::default()), expected);
    let composite = bridged(&cinematic_composite(library()).expect("a composite"));
    let standard = Composite::standard();
    let operations = |composite: &Composite| {
        composite
            .nodes
            .iter()
            .map(|node| node.operation.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(operations(&composite), operations(&standard));
}

fn bridged(graph: &rikka_shader_graph::Graph) -> Composite {
    struct Natives;
    impl rikka_shader_compile::Resolve for Natives {
        fn pixel(
            &mut self,
            _: &rikka_shader_graph::PixelStep,
        ) -> Option<rikka_renderer_3d::EffectId> {
            None
        }

        fn builtin(
            &mut self,
            _: rikka_renderer_3d::BuiltinEffect,
        ) -> Option<rikka_renderer_3d::EffectId> {
            None
        }

        fn socket(&mut self, at: &rikka_shader_graph::SocketRef) -> Option<SocketValue> {
            let _ = at;
            None
        }

        fn texture(&mut self, _: &str) -> Option<rikka_renderer_3d::TextureBinding> {
            None
        }
    }
    let plan = graph.composite(library()).expect("it plans");
    let sockets = |at: &rikka_shader_graph::SocketRef| graph.socket(at).cloned();
    let records = plan.evaluate(&rikka_shader_graph::Env {
        time: 0.0,
        sockets: &sockets,
    });
    rikka_shader_compile::composite(&plan, &records, &mut Natives).expect("it bridges")
}
