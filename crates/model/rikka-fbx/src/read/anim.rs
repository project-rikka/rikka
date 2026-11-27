use rikka_model_core::glam::{Quat, Vec3};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::document::Node;
use crate::euler;
use crate::read::curve::Curve as Keyframes;
use crate::read::graph::{Graph, synthetic};
use crate::read::scene::Scene;
use rikka_model_core::EulerOrder;
use rikka_model_core::ImportCtx;
use rikka_model_core::ModelBuilder;
use rikka_model_core::NodeId;
use rikka_model_core::Result;
use rikka_model_core::Transform;
use rikka_model_core::bake::{self, Subdivision, TransformParts, TransformSamples};
use rikka_model_core::{Animation, Channel, Interpolation, Track, Weights};

use crate::PARALLEL;

const TRANSLATION: &str = "Lcl Translation";
const ROTATION: &str = "Lcl Rotation";
const SCALING: &str = "Lcl Scaling";
const DEFORM: &str = "DeformPercent";

const REFINEMENT: Subdivision = Subdivision::Halves { rounds: 4 };
const TAKES_PER_CHECKPOINT: usize = 16;

#[derive(Clone, Copy)]
pub(crate) struct BlendShapeSlot {
    pub node: NodeId,
    pub slot: usize,
    pub count: usize,
}

pub(crate) fn read(
    records: &[Node],
    graph: &Graph<'_>,
    scene: &Scene,
    shaped: &FxHashMap<i64, BlendShapeSlot>,
    ktime: i64,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<()> {
    let mut takes = modern(graph, ktime);
    takes.extend(legacy(records, ktime));
    for batch in takes.chunks(TAKES_PER_CHECKPOINT) {
        ctx.checkpoint()?;
        let baked = PARALLEL.map(batch, |take| bake(take, scene, shaped));
        for animation in baked.into_iter().flatten() {
            builder.add_animation(animation);
        }
    }
    Ok(())
}

#[derive(Clone, Default)]
struct Driven {
    curves: [Option<Keyframes>; 3],
    defaults: [Option<f32>; 3],
}

impl Driven {
    fn at(&self, time: f64, base: Vec3) -> Vec3 {
        let value = |slot: usize| {
            let fallback = self.defaults[slot].unwrap_or(base[slot]);
            match &self.curves[slot] {
                Some(curve) => curve.evaluate(time, fallback),
                None => fallback,
            }
        };
        Vec3::new(value(0), value(1), value(2))
    }

    fn times(&self) -> impl Iterator<Item = f64> + '_ {
        self.curves.iter().flatten().flat_map(Keyframes::times)
    }
}

#[derive(Default)]
struct Layer {
    weight: f32,
    additive: bool,
    blended: bool,
    compose_rotation: bool,
    compose_scale: bool,
    driven: FxHashMap<i64, FxHashMap<String, Driven>>,
}

impl Layer {
    fn plain() -> Self {
        Self {
            weight: 1.0,
            compose_rotation: true,
            compose_scale: true,
            ..Self::default()
        }
    }

    fn drive(&mut self, target: i64, property: &str, driven: Driven) {
        self.driven
            .entry(target)
            .or_default()
            .insert(property.to_owned(), driven);
    }

    fn driven(&self, target: i64, property: &str) -> Option<&Driven> {
        self.driven.get(&target)?.get(property)
    }
}

struct Take {
    name: String,
    start: Option<f64>,
    stop: Option<f64>,
    layers: Vec<Layer>,
}

fn modern(graph: &Graph<'_>, ktime: i64) -> Vec<Take> {
    let mut takes = Vec::new();
    for stack in graph
        .objects()
        .filter(|object| object.kind == "AnimationStack")
    {
        let seconds = |name: &str| {
            stack
                .props
                .integer(name)
                .map(|ticks| ticks as f64 / ktime as f64)
        };
        let mut take = Take {
            name: stack.name.clone(),
            start: seconds("LocalStart"),
            stop: seconds("LocalStop"),
            layers: Vec::new(),
        };
        for (layer_object, _) in graph.sources(stack.id) {
            if layer_object.kind != "AnimationLayer" {
                continue;
            }
            let mut layer = Layer {
                weight: (layer_object.props.real("Weight", 100.0) / 100.0).clamp(0.0, 1.0),
                compose_rotation: layer_object.props.integer("RotationAccumulationMode") != Some(1),
                compose_scale: layer_object.props.integer("ScaleAccumulationMode") != Some(1),
                ..Layer::default()
            };
            match layer_object.props.integer("BlendMode").unwrap_or(0) {
                0 => {
                    layer.blended = true;
                    layer.additive = true;
                }
                2 => layer.blended = true,
                _ => {}
            }

            for (node, _) in graph.sources(layer_object.id) {
                if node.kind != "AnimationCurveNode" {
                    continue;
                }
                let mut driven = Driven::default();
                for name in node.props.names() {
                    if let Some(component) = name.strip_prefix("d|") {
                        driven.defaults[slot(component)] =
                            node.props.number(name).map(|value| value as f32);
                    }
                }
                for (curve, property) in graph.sources(node.id) {
                    if curve.kind == "AnimationCurve" {
                        driven.curves[slot(property.unwrap_or_default())] =
                            Some(Keyframes::read(curve.node, ktime));
                    }
                }
                for (target, property) in graph.destinations(node.id) {
                    if let Some(property) = property {
                        layer.drive(target.id, property, driven.clone());
                    }
                }
            }
            take.layers.push(layer);
        }
        takes.push(take);
    }
    takes
}

fn legacy(records: &[Node], ktime: i64) -> Vec<Take> {
    let mut takes = Vec::new();
    for block in records.iter().filter(|node| node.name == "Takes") {
        for node in block.children_named("Take") {
            let time = node.child("LocalTime");
            let mut layer = Layer::plain();
            for model in node.children_named("Model") {
                let Some(named) = model.text(0) else {
                    continue;
                };
                let target = synthetic(named);
                for channel in model.children_named("Channel") {
                    read_channel(channel, target, &mut layer, ktime);
                }
            }
            takes.push(Take {
                name: node.text(0).unwrap_or("Take").to_owned(),
                start: time
                    .and_then(|node| node.integer(0))
                    .map(|ticks| ticks as f64 / ktime as f64),
                stop: time
                    .and_then(|node| node.integer(1))
                    .map(|ticks| ticks as f64 / ktime as f64),
                layers: vec![layer],
            });
        }
    }
    takes
}

fn read_channel(channel: &Node, target: i64, layer: &mut Layer, ktime: i64) {
    let Some(name) = channel.text(0) else {
        return;
    };
    if name == "Transform" {
        for axis_channel in channel.children_named("Channel") {
            let property = match axis_channel.text(0) {
                Some("T") => TRANSLATION,
                Some("R") => ROTATION,
                Some("S") => SCALING,
                _ => continue,
            };
            layer.drive(target, property, curves(axis_channel, ktime));
        }
        return;
    }
    let property = name.strip_suffix(" (Shape)").unwrap_or(name);
    layer.drive(target, property, curves(channel, ktime));
}

fn curves(channel: &Node, ktime: i64) -> Driven {
    let version = channel.child_integer("KeyVer").unwrap_or(4005);
    let mut driven = Driven::default();
    let single = channel.child("Key").is_some() || channel.child("Default").is_some();
    if single {
        driven.defaults[0] = channel.child_number("Default").map(|value| value as f32);
        driven.curves[0] = Some(Keyframes::read_legacy(channel, ktime, version));
        return driven;
    }
    for (index, axis_channel) in channel.children_named("Channel").enumerate().take(3) {
        if axis_channel.child("Key").is_none() && axis_channel.child("Default").is_none() {
            continue;
        }
        let at = match axis_channel.text(0) {
            Some(name) => slot(name),
            None => index,
        };
        let version = axis_channel.child_integer("KeyVer").unwrap_or(version);
        driven.defaults[at] = axis_channel
            .child_number("Default")
            .map(|value| value as f32);
        driven.curves[at] = Some(Keyframes::read_legacy(axis_channel, ktime, version));
    }
    driven
}

fn slot(component: &str) -> usize {
    match component.rsplit('|').next().unwrap_or(component) {
        "Y" => 1,
        "Z" => 2,
        _ => 0,
    }
}

fn bake(take: &Take, scene: &Scene, shaped: &FxHashMap<i64, BlendShapeSlot>) -> Option<Animation> {
    let mut moved: Vec<i64> = Vec::new();
    let mut seen: FxHashSet<i64> = FxHashSet::default();
    let mut weighted: FxHashMap<NodeId, Vec<i64>> = FxHashMap::default();

    for layer in &take.layers {
        for (target, driven) in &layer.driven {
            for property in driven.keys() {
                match property.as_str() {
                    TRANSLATION | ROTATION | SCALING if scene.nodes.contains_key(target) => {
                        if seen.insert(*target) {
                            moved.push(*target);
                        }
                    }
                    DEFORM => {
                        if let Some(shape_slot) = shaped.get(target) {
                            let channels = weighted.entry(shape_slot.node).or_default();
                            if !channels.contains(target) {
                                channels.push(*target);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let mut animation = Animation::new(take.name.clone());
    animation.channels.extend(
        PARALLEL
            .map(&moved, |target| place(take, scene, *target))
            .into_iter()
            .flatten(),
    );
    animation.channels.extend(
        weighted
            .iter()
            .filter_map(|(node, channels)| blend(take, shaped, *node, channels)),
    );
    (!animation.channels.is_empty()).then_some(animation)
}

fn place(take: &Take, scene: &Scene, target: i64) -> Vec<Channel> {
    let Some(&node) = scene.nodes.get(&target) else {
        return Vec::new();
    };
    let pipeline = scene.pipelines.get(&target).copied().unwrap_or_default();
    let rest = scene.statics.get(&target).copied().unwrap_or_default();
    let order = pipeline.order;

    let driving: Vec<(&Layer, usize, [Option<&Driven>; 3])> = take
        .layers
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            let parts =
                [TRANSLATION, ROTATION, SCALING].map(|property| layer.driven(target, property));
            (layer, index, parts)
        })
        .filter(|(_, _, parts)| parts.iter().any(Option::is_some))
        .collect();
    if driving.is_empty() {
        return Vec::new();
    }

    let sample = |time: f64| -> Transform {
        let mut local = rest;
        for &(layer, index, [translation, rotation, scaling]) in &driving {
            if let Some(driven) = translation {
                let value = driven.at(time, local.0);
                local.0 = laid(layer, index, order, Part::Plain, local.0, value);
            }
            if let Some(driven) = rotation {
                let value = driven.at(time, local.1);
                local.1 = laid(layer, index, order, Part::Rotation, local.1, value);
            }
            if let Some(driven) = scaling {
                let value = driven.at(time, local.2);
                local.2 = laid(layer, index, order, Part::Scaling, local.2, value);
            }
        }
        pipeline.place(local.0, local.1, local.2)
    };

    let mut times: Vec<f64> = driving
        .iter()
        .flat_map(|(_, _, parts)| parts.iter().flatten().flat_map(|driven| driven.times()))
        .collect();
    let Some(start) = span(&mut times, take) else {
        return Vec::new();
    };
    let baked = bake::adaptive(times, REFINEMENT, sample);
    let mut samples = TransformSamples::with_capacity(baked.len());
    for (time, place) in baked {
        samples.push((time - start) as f32, place);
    }
    let driven = |slot: usize| driving.iter().any(|(_, _, parts)| parts[slot].is_some());
    samples.channels(
        node,
        TransformParts {
            translation: driven(0),
            rotation: driven(1),
            scale: driven(2),
        },
    )
}

fn blend(
    take: &Take,
    shaped: &FxHashMap<i64, BlendShapeSlot>,
    node: NodeId,
    channels: &[i64],
) -> Option<Channel> {
    let count = channels
        .first()
        .and_then(|target| shaped.get(target))
        .map(|shape_slot| shape_slot.count)?;

    let sample = |target: i64, time: f64| -> f32 {
        let mut weight = 0.0;
        for (index, layer) in take.layers.iter().enumerate() {
            if let Some(driven) = layer.driven(target, DEFORM) {
                let value = driven.at(time, Vec3::splat(weight));
                weight = laid(
                    layer,
                    index,
                    EulerOrder::Xyz,
                    Part::Plain,
                    Vec3::splat(weight),
                    value,
                )
                .x;
            }
        }
        weight / 100.0
    };

    let mut times: Vec<f64> = take
        .layers
        .iter()
        .flat_map(|layer| {
            channels
                .iter()
                .filter_map(|target| layer.driven(*target, DEFORM))
                .flat_map(Driven::times)
        })
        .collect();
    let start = span(&mut times, take)?;

    let mut values = vec![0.0; times.len() * count];
    for (frame, &time) in times.iter().enumerate() {
        for &target in channels {
            let Some(shape_slot) = shaped
                .get(&target)
                .filter(|shape_slot| shape_slot.slot < count)
            else {
                continue;
            };
            values[frame * count + shape_slot.slot] = sample(target, time);
        }
    }
    Some(Channel {
        target: node,
        track: Track::Weights(Weights {
            times: times.iter().map(|&time| (time - start) as f32).collect(),
            values,
            count,
            interpolation: Interpolation::Linear,
        }),
    })
}

fn span(times: &mut Vec<f64>, take: &Take) -> Option<f64> {
    let first = take.start.unwrap_or(f64::NEG_INFINITY);
    let last = take.stop.unwrap_or(f64::INFINITY);
    times.retain(|time| *time >= first && *time <= last && time.is_finite());
    if let Some(start) = take.start {
        times.push(start);
    }
    if let Some(stop) = take.stop {
        times.push(stop);
    }
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let start = times.first().copied()?;
    (times.len() > 1).then_some(start)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Plain,
    Rotation,
    Scaling,
}

fn laid(
    layer: &Layer,
    index: usize,
    order: EulerOrder,
    part: Part,
    base: Vec3,
    value: Vec3,
) -> Vec3 {
    if index == 0 {
        return value;
    }
    let weight = layer.weight;
    let power = |base: f32, exponent: f32| match exponent {
        exponent if exponent <= 0.0 => 1.0,
        exponent if exponent >= 1.0 => base,
        exponent => base.signum() * base.abs().powf(exponent),
    };
    match (layer.additive, layer.blended) {
        (true, _) => match part {
            Part::Scaling if layer.compose_scale => Vec3::new(
                base.x * power(value.x, weight),
                base.y * power(value.y, weight),
                base.z * power(value.z, weight),
            ),
            Part::Rotation if layer.compose_rotation => {
                let over = Quat::IDENTITY.slerp(euler::quaternion(order, value), weight);
                euler::angles(order, euler::quaternion(order, base) * over)
            }
            _ => base + value * weight,
        },
        (false, true) => match part {
            Part::Scaling if layer.compose_scale => Vec3::new(
                power(base.x, 1.0 - weight) * power(value.x, weight),
                power(base.y, 1.0 - weight) * power(value.y, weight),
                power(base.z, 1.0 - weight) * power(value.z, weight),
            ),
            Part::Rotation if layer.compose_rotation => euler::angles(
                order,
                euler::quaternion(order, base).slerp(euler::quaternion(order, value), weight),
            ),
            _ => base.lerp(value, weight),
        },
        (false, false) => value,
    }
}
