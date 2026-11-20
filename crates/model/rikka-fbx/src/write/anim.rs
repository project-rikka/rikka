use rikka_model_core::glam::{Quat, Vec3};

use crate::KTIME;
use crate::document::{Node, Value};
use crate::euler;
use crate::write::deform::BlendChannels;
use crate::write::scene::NodeIds;
use crate::write::{Composer, named, prop};
use rikka_model_core::EulerOrder;
use rikka_model_core::ExportCtx;
use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::{Interpolation, Track};

const DEFORM: &str = "DeformPercent";

const HOLD: i32 = 0x2;
const LINEAR: i32 = 0x4;
const CUBIC: i32 = 0x8 | 0x400 | 0x800;

pub(crate) fn write(
    model: &Model,
    composer: &mut Composer,
    node_ids: &NodeIds,
    shaped: &BlendChannels,
    ctx: &mut ExportCtx<'_>,
) -> Result<()> {
    for animation in &model.animations {
        ctx.checkpoint()?;
        let stack = composer.id();
        let stop = (f64::from(animation.duration()) * KTIME as f64) as i64;
        composer.add(
            "AnimationStack",
            Node::new("AnimationStack")
                .with_i64(stack)
                .with_text(named(&animation.name, "AnimStack"))
                .with_text("")
                .adding(
                    Node::new("Properties70")
                        .adding(prop("LocalStart", "KTime", "Time", "").with_i64(0))
                        .adding(prop("LocalStop", "KTime", "Time", "").with_i64(stop))
                        .adding(prop("ReferenceStart", "KTime", "Time", "").with_i64(0))
                        .adding(prop("ReferenceStop", "KTime", "Time", "").with_i64(stop)),
                ),
        );

        let layer = composer.id();
        composer.add(
            "AnimationLayer",
            Node::new("AnimationLayer")
                .with_i64(layer)
                .with_text(named("BaseLayer", "AnimLayer"))
                .with_text(""),
        );
        composer.connect(layer, stack);

        for channel in &animation.channels {
            drive(model, composer, node_ids, shaped, layer, channel);
        }
    }
    Ok(())
}

fn drive(
    model: &Model,
    composer: &mut Composer,
    node_ids: &NodeIds,
    shaped: &BlendChannels,
    layer: i64,
    channel: &rikka_model_core::Channel,
) {
    let node = node_ids.node(channel.target);
    match &channel.track {
        Track::Translation(curve) => {
            triple(
                composer,
                layer,
                node,
                Part::Translation,
                &SampledCurve::from_curve(curve),
            );
        }
        Track::Scale(curve) => {
            triple(
                composer,
                layer,
                node,
                Part::Scaling,
                &SampledCurve::from_curve(curve),
            );
        }
        Track::Rotation(curve) => {
            triple(
                composer,
                layer,
                node,
                Part::Rotation,
                &SampledCurve::turning(curve),
            );
        }
        Track::Weights(weights) => {
            let Some(mesh) = model.node(channel.target).mesh else {
                return;
            };
            for slot in 0..weights.count {
                let Some(target) = shaped.channel(mesh, slot) else {
                    continue;
                };
                let values: Vec<f32> = (0..weights.times.len())
                    .filter_map(|index| Some(weights.key(index)?.get(slot)? * 100.0))
                    .collect();
                single(
                    composer,
                    layer,
                    target,
                    &weights.times,
                    &values,
                    weights.interpolation,
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Part {
    Translation,
    Rotation,
    Scaling,
}

impl Part {
    fn named(self) -> (&'static str, &'static str) {
        match self {
            Self::Translation => ("T", "Lcl Translation"),
            Self::Rotation => ("R", "Lcl Rotation"),
            Self::Scaling => ("S", "Lcl Scaling"),
        }
    }
}

struct SampledCurve<'a> {
    times: &'a [f32],
    values: Vec<Vec3>,
    interpolation: Interpolation,
    tangents: Option<Vec<(Vec3, Vec3)>>,
}

impl<'a> SampledCurve<'a> {
    fn from_curve(curve: &'a rikka_model_core::Curve<Vec3>) -> Self {
        Self {
            times: &curve.times,
            values: (0..curve.times.len())
                .filter_map(|index| curve.key(index))
                .collect(),
            interpolation: curve.interpolation,
            tangents: match curve.interpolation {
                Interpolation::CubicSpline => Some(
                    (0..curve.times.len())
                        .map(|index| {
                            let out = curve
                                .values
                                .get(index * 3 + 2)
                                .copied()
                                .unwrap_or(Vec3::ZERO);
                            let next = curve
                                .values
                                .get((index + 1) * 3)
                                .copied()
                                .unwrap_or(Vec3::ZERO);
                            (out, next)
                        })
                        .collect(),
                ),
                _ => None,
            },
        }
    }

    fn turning(curve: &'a rikka_model_core::Curve<Quat>) -> Self {
        Self {
            times: &curve.times,
            values: angles(curve),
            interpolation: match curve.interpolation {
                Interpolation::Step => Interpolation::Step,
                _ => Interpolation::Linear,
            },
            tangents: None,
        }
    }
}

fn angles(curve: &rikka_model_core::Curve<Quat>) -> Vec<Vec3> {
    let mut angles = Vec::with_capacity(curve.times.len());
    let mut previous = Vec3::ZERO;
    for index in 0..curve.times.len() {
        let Some(rotation) = curve.key(index) else {
            continue;
        };
        let mut angle = euler::angles(EulerOrder::Xyz, rotation.normalize());
        for slot in 0..3 {
            let turns = ((previous[slot] - angle[slot]) / 360.0).round();
            angle[slot] += turns * 360.0;
        }
        previous = angle;
        angles.push(angle);
    }
    angles
}

fn triple(composer: &mut Composer, layer: i64, target: i64, part: Part, baked: &SampledCurve<'_>) {
    let (name, property) = part.named();
    let curve_node = composer.id();
    let first = baked.values.first().copied().unwrap_or(Vec3::ZERO);
    composer.add(
        "AnimationCurveNode",
        Node::new("AnimationCurveNode")
            .with_i64(curve_node)
            .with_text(named(name, "AnimCurveNode"))
            .with_text("")
            .adding(
                Node::new("Properties70")
                    .adding(component("d|X", first.x))
                    .adding(component("d|Y", first.y))
                    .adding(component("d|Z", first.z)),
            ),
    );
    composer.connect(curve_node, layer);
    composer.connect_property(curve_node, target, property);

    for (slot, component) in ["d|X", "d|Y", "d|Z"].into_iter().enumerate() {
        let curve = composer.id();
        let along: Vec<f32> = baked.values.iter().map(|value| value[slot]).collect();
        let tangents = baked.tangents.as_ref().map(|tangents| {
            tangents
                .iter()
                .map(|(out, next)| (out[slot], next[slot]))
                .collect()
        });
        composer.add(
            "AnimationCurve",
            keyframes(curve, baked.times, &along, baked.interpolation, tangents),
        );
        composer.connect_property(curve, curve_node, component);
    }
}

fn single(
    composer: &mut Composer,
    layer: i64,
    target: i64,
    times: &[f32],
    values: &[f32],
    interpolation: Interpolation,
) {
    let property = DEFORM;
    let curve_node = composer.id();
    composer.add(
        "AnimationCurveNode",
        Node::new("AnimationCurveNode")
            .with_i64(curve_node)
            .with_text(named(property, "AnimCurveNode"))
            .with_text("")
            .adding(Node::new("Properties70").adding(component(
                "d|DeformPercent",
                values.first().copied().unwrap_or(0.0),
            ))),
    );
    composer.connect(curve_node, layer);
    composer.connect_property(curve_node, target, property);

    let curve = composer.id();
    composer.add(
        "AnimationCurve",
        keyframes(curve, times, values, interpolation, None),
    );
    composer.connect_property(curve, curve_node, "d|DeformPercent");
}

fn component(name: &str, value: f32) -> Node {
    prop(name, "Number", "", "A").with_f64(f64::from(value))
}

fn keyframes(
    id: i64,
    times: &[f32],
    values: &[f32],
    interpolation: Interpolation,
    tangents: Option<Vec<(f32, f32)>>,
) -> Node {
    let count = times.len().min(values.len());
    let flags = match interpolation {
        Interpolation::Step => HOLD,
        Interpolation::Linear => LINEAR,
        Interpolation::CubicSpline => CUBIC,
    };
    let mut attributes: Vec<f32> = Vec::with_capacity(count * 4);
    let mut repeats: Vec<i32> = Vec::with_capacity(count);
    let mut key_flags: Vec<i32> = Vec::with_capacity(count);
    for index in 0..count {
        let (out, next) = tangents
            .as_ref()
            .and_then(|tangents| tangents.get(index).copied())
            .unwrap_or((0.0, 0.0));
        attributes.extend([out, next, 0.0, 0.0]);
        repeats.push(1);
        key_flags.push(flags);
    }

    Node::new("AnimationCurve")
        .with_i64(id)
        .with_text(named("", "AnimCurve"))
        .with_text("")
        .adding(Node::new("Default").with_f64(f64::from(values.first().copied().unwrap_or(0.0))))
        .adding(Node::new("KeyVer").with_i32(4009))
        .adding(
            Node::new("KeyTime").with(Value::I64s(
                times[..count]
                    .iter()
                    .map(|&time| (f64::from(time) * KTIME as f64) as i64)
                    .collect(),
            )),
        )
        .adding(Node::new("KeyValueFloat").with(Value::F32s(values[..count].to_vec())))
        .adding(Node::new("KeyAttrFlags").with(Value::I32s(key_flags)))
        .adding(Node::new("KeyAttrDataFloat").with(Value::F32s(attributes)))
        .adding(Node::new("KeyAttrRefCount").with(Value::I32s(repeats)))
}
