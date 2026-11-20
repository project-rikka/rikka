use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    AgxLook, Bounds, BuiltinEffect, Composite, CompositeInput, DepthOfField, Effect, EffectFamily,
    EffectId, EffectInput, EffectInstance, EffectOutput, EffectStep, EffectUniform, Exposure, Glow,
    Grade, Lens, Literal, Metering, MotionBlur, Operation, ParameterKind, ParameterSpec,
    RenderLayer, ShaderCompiler, TextureBinding, Tone, ToneCurve, Tonemap, Value, Wiring,
};
use rikka_shader_graph::{
    Behavior, Category, CompositePlan, Constant, Datum, Hint, Hosts, Layer, Link, NodeSpec,
    PIXEL_ENTRY, PixelStep, PortSpec, Record, SocketRef, SocketValue, Step, ValueType,
};

use crate::error::Error;
use crate::material::socket_value;

const IMAGE: PortSpec = PortSpec::required("image", ValueType::Image);
const OUTPUT: PortSpec = PortSpec::output("image", ValueType::Image);
const EFFECT_PREFIX: &str = "effect.";
const METERINGS: &[&str] = &["auto", "manual", "physical"];
const TONES: &[&str] = &[
    "gt", "agx", "aces", "neutral", "clamp", "hable", "reinhard", "lottes", "custom",
];
const AGX_LOOKS: &[&str] = &["base", "punchy", "golden"];

fn filter(
    id: &'static str,
    title: &'static str,
    category: Category,
    doc: &'static str,
    mut inputs: Vec<PortSpec>,
) -> NodeSpec {
    inputs.insert(0, IMAGE);
    NodeSpec {
        id,
        title,
        category,
        doc,
        hosts: Hosts::COMPOSITE,
        inputs,
        outputs: vec![OUTPUT],
        behavior: Behavior::Filter,
    }
}

fn unit(name: &'static str, value: f32) -> PortSpec {
    PortSpec::number(name, value).range(0.0, 1.0)
}

fn soft(name: &'static str, value: f32, low: f32, high: f32) -> PortSpec {
    PortSpec::number(name, value).soft(low, high)
}

fn natives() -> Vec<NodeSpec> {
    let exposure = Exposure::default();
    let tonemap = Tonemap::default();
    let curve = tonemap.curve;
    let focus = DepthOfField::default();
    let glow = Glow::default();
    let grade = Grade::default();
    let lens = Lens::default();
    let color = |name: &'static str, value: Vec3| PortSpec::color(name, value.x, value.y, value.z);
    vec![
        filter(
            "filter.exposure",
            "Exposure",
            Category::Lens,
            "Exposes the image by metering it to middle grey, by a fixed amount in stops, or like a camera from aperture, shutter and ISO.",
            vec![
                PortSpec::options("metering", METERINGS, 0),
                soft("compensation", exposure.compensation, -6.0, 6.0),
                soft("adaptation", exposure.adaptation, 0.0, 4.0),
                unit("figure_metering", exposure.figure_metering),
                soft("aperture", exposure.aperture, 0.7, 32.0),
                soft("shutter", exposure.shutter, 1.0 / 8000.0, 1.0),
                soft("iso", exposure.iso, 50.0, 12800.0),
                unit("low_percentile", exposure.low_percentile),
                unit("high_percentile", exposure.high_percentile),
                soft("darkest", exposure.darkest, -12.0, 0.0),
                soft("brightest", exposure.brightest, 0.0, 12.0),
                unit("center_weight", exposure.center_weight),
            ],
        ),
        filter(
            "filter.motion_blur",
            "Motion blur",
            Category::Lens,
            "Smears what moves across the frame along its motion.",
            vec![soft("amount", MotionBlur::default().amount, 0.0, 2.0)],
        ),
        filter(
            "filter.depth_of_field",
            "Depth of field",
            Category::Lens,
            "Blurs what lies off the focus distance like a lens of the given aperture.",
            vec![
                soft("focus", focus.focus, 0.1, 50.0),
                soft("aperture", focus.aperture, 0.7, 22.0),
                soft("sensor", focus.sensor, 4.0, 70.0),
                PortSpec::number("blades", focus.blades as f32)
                    .hint(Hint::Integer)
                    .range(0.0, 12.0),
                PortSpec::number("blade_rotation", focus.blade_rotation).hint(Hint::Angle),
            ],
        ),
        filter(
            "filter.glow",
            "Glow",
            Category::Lens,
            "Bloom, diffusion, streaks, flares and light leaks around bright areas of the image or of source.",
            vec![
                PortSpec::empty(Glow::SOURCE, ValueType::Image),
                soft("bloom", glow.bloom, 0.0, 2.0),
                unit("radius", glow.radius),
                soft("threshold", glow.threshold, -4.0, 8.0),
                color("tint", glow.tint),
                unit("diffusion", glow.diffusion),
                soft("streaks", glow.streaks, 0.0, 2.0),
                color("streak_tint", glow.streak_tint),
                soft("flare", glow.flare, 0.0, 2.0),
                soft("leaks", glow.leaks, 0.0, 2.0),
                color("leak_tint", glow.leak_tint),
                PortSpec::number("blades", glow.blades as f32)
                    .hint(Hint::Integer)
                    .range(0.0, 12.0),
                PortSpec::number("blade_rotation", glow.blade_rotation).hint(Hint::Angle),
            ],
        ),
        filter(
            "filter.grade",
            "Grade",
            Category::Color,
            "Grades scene-referred colour: contrast, saturation, white balance, lift, gamma, gain and split tints.",
            vec![
                soft("contrast", grade.contrast, 0.5, 2.0),
                soft("saturation", grade.saturation, 0.0, 2.0),
                soft("vibrance", grade.vibrance, -1.0, 1.0),
                soft("temperature", grade.temperature, -1.0, 1.0),
                soft("tint", grade.tint, -1.0, 1.0),
                color("lift", grade.lift),
                color("gamma", grade.gamma),
                color("gain", grade.gain),
                color("shadow_tint", grade.shadow_tint),
                color("highlight_tint", grade.highlight_tint),
                soft("black", grade.black, -0.1, 0.1),
                unit("night", grade.night),
            ],
        ),
        filter(
            "filter.tonemap",
            "Tone map",
            Category::Color,
            "Maps scene light onto the display with a tone curve, optionally keeping hues; what follows it works in display colour.",
            vec![
                PortSpec::options("tone", TONES, 0),
                PortSpec::options("look", AGX_LOOKS, 1),
                unit("preserve_hue", tonemap.preserve_hue),
                soft("peak", curve.peak, 0.5, 4.0),
                soft("contrast", curve.contrast, 0.25, 3.0),
                unit("toe", curve.toe),
                unit("linear", curve.linear),
                soft("toe_power", curve.toe_power, 0.5, 3.0),
            ],
        ),
        filter(
            "filter.lens",
            "Lens finish",
            Category::Lens,
            "Display-referred lens traits: chromatic aberration, vignette, grain and sharpening.",
            vec![
                soft("chromatic_aberration", lens.chromatic_aberration, 0.0, 2.0),
                unit("vignette", lens.vignette),
                unit("grain", lens.grain),
                unit("sharpen", lens.sharpen),
            ],
        ),
    ]
}

fn leak(text: &str) -> &'static str {
    Box::leak(text.to_owned().into_boxed_str())
}

fn ranged(port: PortSpec, spec: &ParameterSpec) -> PortSpec {
    match (spec.range, spec.soft_range) {
        (Some(Bounds { low, high }), _) => port.range(low, high),
        (None, Some(Bounds { low, high })) => port.soft(low, high),
        (None, None) => port,
    }
}

fn parameter_port(spec: &ParameterSpec) -> Option<PortSpec> {
    let port = typed_port(spec)?;
    Some(match spec.hints.contains(&rikka_renderer_3d::Hint::Angle) {
        true => port.hint(Hint::Angle),
        false => port,
    })
}

fn typed_port(spec: &ParameterSpec) -> Option<PortSpec> {
    let name = leak(&spec.name);
    let vector = |ty: ValueType, values: &[f32]| {
        PortSpec::value(
            name,
            ty,
            Constant::from_slice(values).expect("a literal holds one to four values"),
        )
    };
    Some(match (&spec.kind, spec.default) {
        (ParameterKind::Float, Literal::Float(value)) => {
            ranged(PortSpec::number(name, value), spec)
        }
        (ParameterKind::Vec2, Literal::Vec2(value)) => vector(ValueType::Float2, &value),
        (ParameterKind::Vec3, Literal::Vec3(value)) => vector(ValueType::Float3, &value),
        (ParameterKind::Vec4, Literal::Vec4(value)) => vector(ValueType::Float4, &value),
        (ParameterKind::Color { alpha: false, .. }, Literal::Vec3(value)) => {
            vector(ValueType::Float3, &value).hint(Hint::Color)
        }
        (ParameterKind::Color { alpha: true, .. }, Literal::Vec4(value)) => {
            vector(ValueType::Float4, &value).hint(Hint::Color)
        }
        (ParameterKind::Int, Literal::Int(value)) => {
            ranged(PortSpec::number(name, value as f32), spec).hint(Hint::Integer)
        }
        (ParameterKind::UInt, Literal::UInt(value)) => {
            ranged(PortSpec::number(name, value as f32), spec).hint(Hint::Integer)
        }
        (ParameterKind::Bool, Literal::Bool(value)) => PortSpec::toggle(name, value),
        (ParameterKind::Enum { options }, Literal::Enum(chosen)) => {
            let options: Vec<&'static str> = options.iter().map(|option| leak(option)).collect();
            PortSpec::options(name, Box::leak(options.into_boxed_slice()), chosen as usize)
        }
        (ParameterKind::Texture { kind, .. }, _) => {
            PortSpec::empty(name, ValueType::Texture).hint(Hint::Texture(*kind))
        }
        _ => return None,
    })
}

fn category(family: EffectFamily) -> Category {
    match family {
        EffectFamily::Blur | EffectFamily::Detail | EffectFamily::Time => Category::Filter,
        EffectFamily::Glare | EffectFamily::Lens | EffectFamily::Film | EffectFamily::Distort => {
            Category::Lens
        }
        EffectFamily::Color => Category::Color,
        EffectFamily::Stylize => Category::Stylize,
        EffectFamily::Layers => Category::Layer,
    }
}

fn builtin(builtin: BuiltinEffect) -> Result<NodeSpec, Error> {
    let effect = builtin.effect();
    let schema = effect.schema()?;
    let sockets = effect.sockets().skip(1).map(|(name, wiring)| match wiring {
        Wiring::Required => PortSpec::required(leak(name), ValueType::Image),
        Wiring::Optional => PortSpec::empty(leak(name), ValueType::Image),
    });
    let inputs = sockets
        .chain(schema.parameters().iter().filter_map(parameter_port))
        .collect();
    Ok(filter(
        leak(&format!("{EFFECT_PREFIX}{}", builtin.name())),
        builtin.title(),
        category(builtin.family()),
        builtin.doc(),
        inputs,
    ))
}

pub fn filters() -> Result<Vec<NodeSpec>, Error> {
    let mut specs = natives();
    for effect in BuiltinEffect::ALL {
        specs.push(builtin(effect)?);
    }
    Ok(specs)
}

pub fn builtin_of(id: &str) -> Option<BuiltinEffect> {
    BuiltinEffect::from_name(id.strip_prefix(EFFECT_PREFIX)?)
}

pub fn pixel_effect(
    step: &PixelStep,
    compiler: &ShaderCompiler,
    name: &str,
) -> Result<Effect, Error> {
    let code = compiler.effect_code(name, &step.source, PIXEL_ENTRY)?;
    let mut effect = Effect::new(name);
    let mut pass = EffectStep::new(code);
    for (member, _) in &step.images {
        effect = effect.input(member.clone());
        pass = pass.read(member.clone(), EffectInput::Input(member.clone()));
    }
    pass = pass
        .write("target", EffectOutput::Image)
        .uniform("size", EffectUniform::Size);
    if step.timed {
        pass = pass.uniform("time", EffectUniform::Time);
    }
    Ok(effect.step(pass))
}

pub trait Resolve {
    fn pixel(&mut self, step: &PixelStep) -> Option<EffectId>;

    fn builtin(&mut self, effect: BuiltinEffect) -> Option<EffectId>;

    fn socket(&mut self, at: &SocketRef) -> Option<SocketValue>;

    fn texture(&mut self, path: &str) -> Option<TextureBinding>;
}

fn layer(layer: Layer) -> RenderLayer {
    match layer {
        Layer::Image => RenderLayer::Image,
        Layer::Depth => RenderLayer::Depth,
        Layer::Normals => RenderLayer::Normals,
        Layer::Velocity => RenderLayer::Velocity,
        Layer::Layers => RenderLayer::Layers,
        Layer::Emission => RenderLayer::Emission,
        Layer::Occlusion => RenderLayer::Occlusion,
        Layer::Previous => RenderLayer::Previous,
    }
}

fn float(record: &Record, name: &str, fallback: f32) -> f32 {
    record.float(name).unwrap_or(fallback)
}

fn vector(record: &Record, name: &str, fallback: Vec3) -> Vec3 {
    record
        .number(name)
        .map(|value| {
            let value = value.resized(3);
            Vec3::new(value.component(0), value.component(1), value.component(2))
        })
        .unwrap_or(fallback)
}

fn whole(record: &Record, name: &str, fallback: u32) -> u32 {
    record
        .float(name)
        .map_or(fallback, |value| value.round().max(0.0) as u32)
}

fn native(record: &Record) -> Option<Operation> {
    Some(match record.id.as_str() {
        "filter.exposure" => {
            let base = Exposure::default();
            Operation::Exposure(Exposure {
                compensation: float(record, "compensation", base.compensation),
                adaptation: float(record, "adaptation", base.adaptation),
                figure_metering: float(record, "figure_metering", base.figure_metering),
                metering: match whole(record, "metering", 0) {
                    0 => Metering::Auto,
                    1 => Metering::Manual,
                    _ => Metering::Physical,
                },
                aperture: float(record, "aperture", base.aperture),
                shutter: float(record, "shutter", base.shutter),
                iso: float(record, "iso", base.iso),
                low_percentile: float(record, "low_percentile", base.low_percentile),
                high_percentile: float(record, "high_percentile", base.high_percentile),
                darkest: float(record, "darkest", base.darkest),
                brightest: float(record, "brightest", base.brightest),
                center_weight: float(record, "center_weight", base.center_weight),
            })
        }
        "filter.motion_blur" => Operation::MotionBlur(MotionBlur {
            amount: float(record, "amount", MotionBlur::default().amount),
        }),
        "filter.depth_of_field" => {
            let base = DepthOfField::default();
            Operation::DepthOfField(DepthOfField {
                focus: float(record, "focus", base.focus),
                aperture: float(record, "aperture", base.aperture),
                sensor: float(record, "sensor", base.sensor),
                blades: whole(record, "blades", base.blades),
                blade_rotation: float(record, "blade_rotation", base.blade_rotation),
            })
        }
        "filter.glow" => {
            let base = Glow::default();
            Operation::Glow(Glow {
                bloom: float(record, "bloom", base.bloom),
                radius: float(record, "radius", base.radius),
                threshold: float(record, "threshold", base.threshold),
                tint: vector(record, "tint", base.tint),
                diffusion: float(record, "diffusion", base.diffusion),
                streaks: float(record, "streaks", base.streaks),
                streak_tint: vector(record, "streak_tint", base.streak_tint),
                flare: float(record, "flare", base.flare),
                leaks: float(record, "leaks", base.leaks),
                leak_tint: vector(record, "leak_tint", base.leak_tint),
                blades: whole(record, "blades", base.blades),
                blade_rotation: float(record, "blade_rotation", base.blade_rotation),
            })
        }
        "filter.grade" => {
            let base = Grade::default();
            Operation::Grade(Grade {
                contrast: float(record, "contrast", base.contrast),
                saturation: float(record, "saturation", base.saturation),
                vibrance: float(record, "vibrance", base.vibrance),
                temperature: float(record, "temperature", base.temperature),
                tint: float(record, "tint", base.tint),
                lift: vector(record, "lift", base.lift),
                gamma: vector(record, "gamma", base.gamma),
                gain: vector(record, "gain", base.gain),
                shadow_tint: vector(record, "shadow_tint", base.shadow_tint),
                highlight_tint: vector(record, "highlight_tint", base.highlight_tint),
                black: float(record, "black", base.black),
                night: float(record, "night", base.night),
            })
        }
        "filter.tonemap" => {
            let base = Tonemap::default();
            Operation::Tonemap(Tonemap {
                tone: match whole(record, "tone", 0) {
                    0 => Tone::Gt,
                    1 => Tone::Agx,
                    2 => Tone::Aces,
                    3 => Tone::Neutral,
                    4 => Tone::Clamp,
                    5 => Tone::Hable,
                    6 => Tone::Reinhard,
                    7 => Tone::Lottes,
                    _ => Tone::Custom,
                },
                look: match whole(record, "look", 1) {
                    0 => AgxLook::Base,
                    1 => AgxLook::Punchy,
                    _ => AgxLook::Golden,
                },
                preserve_hue: float(record, "preserve_hue", base.preserve_hue),
                curve: ToneCurve {
                    peak: float(record, "peak", base.curve.peak),
                    contrast: float(record, "contrast", base.curve.contrast),
                    toe: float(record, "toe", base.curve.toe),
                    linear: float(record, "linear", base.curve.linear),
                    toe_power: float(record, "toe_power", base.curve.toe_power),
                },
            })
        }
        "filter.lens" => {
            let base = Lens::default();
            Operation::Lens(Lens {
                chromatic_aberration: float(
                    record,
                    "chromatic_aberration",
                    base.chromatic_aberration,
                ),
                vignette: float(record, "vignette", base.vignette),
                grain: float(record, "grain", base.grain),
                sharpen: float(record, "sharpen", base.sharpen),
            })
        }
        _ => return None,
    })
}

fn parameter_value(
    spec: &ParameterSpec,
    datum: &Datum,
    resolve: &mut impl Resolve,
) -> Option<Value> {
    match (datum, &spec.kind) {
        (Datum::Asset(path), ParameterKind::Texture { .. }) => Some(Value::Texture(
            path.as_deref().and_then(|path| resolve.texture(path)),
        )),
        (Datum::Number(value), kind) => {
            let first = value.component(0);
            Some(match kind {
                ParameterKind::Float => Value::Float(first),
                ParameterKind::Vec2 => socket_value(ValueType::Float2, *value)?,
                ParameterKind::Vec3 | ParameterKind::Color { alpha: false, .. } => {
                    socket_value(ValueType::Float3, *value)?
                }
                ParameterKind::Vec4 | ParameterKind::Color { alpha: true, .. } => {
                    socket_value(ValueType::Float4, *value)?
                }
                ParameterKind::Int => Value::Int(first.round() as i32),
                ParameterKind::UInt => Value::UInt(first.round().max(0.0) as u32),
                ParameterKind::Bool => Value::Bool(first > 0.5),
                ParameterKind::Enum { options } => {
                    let chosen =
                        (first.round().max(0.0) as usize).min(options.len().saturating_sub(1));
                    Value::Enum(options.get(chosen)?.clone())
                }
                ParameterKind::Texture { .. } => return None,
            })
        }
        _ => None,
    }
}

pub fn composite(
    plan: &CompositePlan,
    records: &[Option<Record>],
    resolve: &mut impl Resolve,
) -> Result<Composite, Error> {
    let mut composite = Composite::new();
    let mut placed: Vec<CompositeInput> = Vec::with_capacity(plan.steps.len());
    let input = |placed: &[CompositeInput], link: Link| match link {
        Link::Layer(found) => CompositeInput::Layer(layer(found)),
        Link::Step(step) => placed[step],
    };
    for (step, record) in plan.steps.iter().zip(records) {
        let (key, images, operation) = match (step, record) {
            (Step::Filter(filter), Some(record)) => {
                let operation = match (native(record), builtin_of(&filter.id)) {
                    (Some(operation), _) => Some(operation),
                    (None, Some(effect)) => match resolve.builtin(effect) {
                        Some(id) => {
                            let schema = effect.effect().schema()?;
                            let mut instance = EffectInstance::new(id);
                            for spec in schema.parameters() {
                                let Some(datum) = record.field(&spec.name) else {
                                    continue;
                                };
                                if let Some(value) = parameter_value(spec, datum, resolve) {
                                    instance = instance.set(spec.name.clone(), value);
                                }
                            }
                            Some(Operation::Effect(instance))
                        }
                        None => None,
                    },
                    (None, None) => None,
                };
                (filter.key, &filter.images, operation)
            }
            (Step::Pixel(pixel), _) => {
                let operation = resolve.pixel(pixel).map(|id| {
                    let mut instance = EffectInstance::new(id);
                    for socket in &pixel.sockets {
                        let value = match resolve.socket(&socket.at) {
                            Some(SocketValue::Number(constant)) => {
                                socket_value(socket.ty, constant)
                            }
                            Some(SocketValue::Asset(path)) => {
                                Some(Value::Texture(resolve.texture(&path)))
                            }
                            None => None,
                        };
                        if let Some(value) = value {
                            instance = instance.set(socket.name.clone(), value);
                        }
                    }
                    Operation::Effect(instance)
                });
                (pixel.key, &pixel.images, operation)
            }
            (Step::Filter(filter), None) => (filter.key, &filter.images, None),
        };
        let through = images
            .first()
            .map_or(CompositeInput::Layer(RenderLayer::Image), |(_, link)| {
                input(&placed, *link)
            });
        placed.push(match operation {
            Some(operation) => {
                let wired: Vec<(String, CompositeInput)> = images
                    .iter()
                    .map(|(socket, link)| (socket.clone(), input(&placed, *link)))
                    .collect();
                let node = composite.node(operation, wired);
                composite.nodes[node].key = key;
                CompositeInput::Node(node)
            }
            None => through,
        });
    }
    composite.output(input(&placed, plan.output));
    Ok(composite)
}
