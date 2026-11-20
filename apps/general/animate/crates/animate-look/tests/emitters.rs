use std::sync::Arc;

use animate_look::{
    GraphRef, Look, LookKey, LookValue, Plans, SocketKey, Target, World, world_nodes,
};
use rikka_mmd::VmdCurve;
use rikka_renderer_3d::Emitter;
use rikka_renderer_3d::glam::Vec3;
use rikka_shader_graph::{
    Builder, Graph, GraphKind, Hint, Input, NodeId, NodeKind, PortDefault, PortSpec, Scope,
    SocketRef, SocketValue,
};

use crate::support::{Lookup, library, wired, world_of};

const SPRITE_ONLY: [&str; 5] = ["sprite", "columns", "rows", "frame_rate", "random_start"];

fn baseline(node: &str, port: &str) -> Vec<(&'static str, SocketValue)> {
    if node != "world.emitter" {
        return Vec::new();
    }
    match port {
        "radius" => vec![("shape", 1.0.into())],
        "extent" => vec![("shape", 2.0.into())],
        "angle" => vec![("shape", 4.0.into())],
        "length" => vec![("shape", 5.0.into())],
        "burst_time" | "burst_spread" | "burst_repeats" => vec![("burst_count", 5.0.into())],
        "burst_interval" => vec![("burst_count", 5.0.into()), ("burst_repeats", 1.0.into())],
        "attractor_position" | "attractor_strength" | "attractor_rate" => {
            vec![("attractor", 1.0.into())]
        }
        "ground_height" | "contact" | "restitution" | "friction" | "ground_fade" => {
            vec![("ground", 1.0.into())]
        }
        "facing_axis" => vec![("facing", 3.0.into())],
        "emission_end" => vec![("emission", 1000.0.into())],
        _ => Vec::new(),
    }
}

fn varied(port: &PortSpec) -> SocketValue {
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

fn rendered(world: &World) -> Vec<Emitter> {
    world.look(&mut Lookup::default()).emitters
}

#[test]
fn every_emitter_socket_reaches_the_rendered_emitter() {
    let emitters = world_nodes()
        .into_iter()
        .filter(|spec| spec.id.starts_with("world.emitter"));
    for spec in emitters {
        for port in spec
            .inputs
            .iter()
            .filter(|port| !SPRITE_ONLY.contains(&port.name))
        {
            let mut sockets = baseline(spec.id, port.name);
            let before = rendered(&world_of(&wired(&[(spec.id, sockets.clone())])));
            sockets.push((port.name, varied(port)));
            let after = rendered(&world_of(&wired(&[(spec.id, sockets)])));
            assert_ne!(before, after, "{} {} is lost", spec.id, port.name);
        }
    }
}

#[test]
fn sprites_load_through_the_resolver() {
    let world = world_of(&wired(&[(
        "world.emitter",
        vec![("sprite", "sprites/smoke.png".into())],
    )]));
    let mut lookup = Lookup::default();
    let emitters = world.look(&mut lookup).emitters;
    assert_eq!(emitters[0].appearance.sprite, None, "waits for the texture");
    assert_eq!(
        lookup.textures,
        [(
            "sprites/smoke.png".to_owned(),
            rikka_renderer_3d::TextureKind::Colour
        )]
    );
    assert_eq!(
        world
            .assets()
            .map(|asset| asset.as_str())
            .collect::<Vec<_>>(),
        ["sprites/smoke.png"]
    );
}

#[test]
fn preset_nodes_match_the_renderer_presets() {
    let calm = Vec3::ZERO;
    let cases: [(&str, Emitter); 10] = [
        (
            "world.emitter.rain",
            Emitter::rain(1.0, Vec3::splat(5.0), calm),
        ),
        (
            "world.emitter.snow",
            Emitter::snow(1.0, Vec3::splat(5.0), calm),
        ),
        ("world.emitter.sparks", Emitter::sparks(1.0, 4.0, calm)),
        ("world.emitter.embers", Emitter::embers(1.0, 0.5, calm)),
        (
            "world.emitter.petals",
            Emitter::petals(1.0, Vec3::splat(4.0), calm),
        ),
        (
            "world.emitter.fireflies",
            Emitter::fireflies(1.0, Vec3::new(4.0, 1.0, 4.0), calm),
        ),
        ("world.emitter.dust", Emitter::dust(1.0, 0.5, calm)),
        ("world.emitter.magic", Emitter::magic(1.0, 0.5, calm)),
        (
            "world.emitter.confetti",
            Emitter::confetti(1.0, Vec3::new(3.0, 4.0, 3.0), calm),
        ),
        (
            "world.emitter.leaves",
            Emitter::leaves(1.0, Vec3::splat(4.0), calm),
        ),
    ];
    for (id, expected) in cases {
        let world = world_of(&wired(&[(id, Vec::new())]));
        assert_eq!(rendered(&world), [expected], "{id}");
    }
}

fn emitter_graph(id: &str, position: Option<Input>) -> (Graph, NodeId) {
    let mut builder = Builder::new(GraphKind::World, library());
    let output = builder.node(NodeKind::Output);
    let emitter = match position {
        Some(input) => {
            let driver = builder.input(input);
            builder.external(id, [("position", driver.into())])
        }
        None => builder.external(id, []),
    };
    let wire = builder.out(emitter, "emitter");
    builder.set(output, "emitters", wire);
    (builder.finish().expect("a world"), emitter)
}

fn evaluated(look: &Look, frame: f32, time: f32) -> World {
    Plans::default()
        .evaluate(look, library(), frame, time)
        .expect("the look evaluates")
        .world
}

#[test]
fn moving_emitters_trace_their_placement() {
    let (driven, _) = emitter_graph("world.emitter.sparks", Some(Input::Time));
    let look = Look {
        world: Arc::new(driven),
        ..Look::default()
    };
    let world = evaluated(&look, 60.0, 2.0);
    let track = &world.emitters[0].track;
    let lifetime = Emitter::sparks(1.0, 4.0, Vec3::ZERO).emission.lifetime;
    assert!(track.len() >= 2 && track.len() <= rikka_renderer_3d::MAX_TRACK_KEYS);
    assert_eq!(track.last().map(|(time, _)| *time), Some(2.0));
    assert!(track[0].0 <= 2.0 - lifetime, "covers every living particle");
    for (time, placement) in track {
        assert!(
            placement.position.abs_diff_eq(Vec3::splat(*time), 1e-5),
            "{time}: {placement:?}"
        );
    }
    let drawn = &rendered(&world)[0];
    assert!(
        drawn
            .track
            .position_at(1.5)
            .abs_diff_eq(Vec3::splat(1.5), 1e-4)
    );

    let (keyed, node) = emitter_graph("world.emitter.rain", None);
    let position = Target::Socket(SocketKey::new(
        GraphRef::World,
        SocketRef::new(Scope::Main, node, "position"),
    ));
    let mut look = Look {
        world: Arc::new(keyed),
        ..Look::default()
    };
    for (frame, x) in [(0, 0.0), (60, 6.0)] {
        look.keys.apply(look.keys.inserting(LookKey {
            target: position.clone(),
            frame,
            value: LookValue::Vec3([x, 0.0, 0.0]),
            curve: VmdCurve::LINEAR,
        }));
    }
    let world = evaluated(&look, 45.0, 1.5);
    let track = &world.emitters[0].track;
    assert!(track.len() >= 2);
    for (time, placement) in track {
        assert!(
            (placement.position.x - 3.0 * time).abs() < 1e-4,
            "{time}: {placement:?}"
        );
    }

    let (still, _) = emitter_graph("world.emitter.rain", None);
    let look = Look {
        world: Arc::new(still),
        ..Look::default()
    };
    assert!(evaluated(&look, 45.0, 1.5).emitters[0].track.is_empty());
}
