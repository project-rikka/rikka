use rikka_slang::TextureKind;

use crate::error::Problem;
use crate::graph::{Graph, GraphKind, Scope};
use crate::library::Library;
use crate::node::{Hint, Input, NodeKind, Port, PortDefault, PortSpec, Ports, context, out, value};
use crate::ops::POSITION;
use crate::types::{Constant, PortType, ValueType};

const RGBA: &[PortSpec] = &[
    out("rgba", PortType::Value(ValueType::Float4)),
    out("rgb", PortType::Value(ValueType::Float3)),
    out("a", PortType::Value(ValueType::Float)),
];

const SHADER: &[PortSpec] = &[PortSpec::output("shader", ValueType::Shader)];

pub(crate) const OUTLINE_PORTS: [&str; 4] = [
    "outline",
    "outline_width",
    "outline_direction",
    "outline_depth",
];

pub(crate) fn output_ports(kind: GraphKind, library: &Library) -> Vec<PortSpec> {
    match kind {
        GraphKind::Material => vec![
            PortSpec::empty("surface", ValueType::Shader),
            PortSpec::number("alpha", 1.0).range(0.0, 1.0),
            PortSpec::vector("displacement", 0.0, 0.0, 0.0),
            PortSpec::empty("outline", ValueType::Shader),
            PortSpec::number("outline_width", 0.0).soft(0.0, 0.01),
            PortSpec::context(
                "outline_direction",
                ValueType::Float3,
                Input::SmoothedNormal,
            ),
            PortSpec::number("outline_depth", 0.0).soft(0.0, 0.01),
        ],
        GraphKind::World => library.world().to_vec(),
        GraphKind::Composite => vec![PortSpec::required("image", ValueType::Image)],
    }
}

pub(crate) fn ports(
    graph: &Graph,
    library: &Library,
    scope: &Scope,
    kind: &NodeKind,
) -> Result<Ports, Problem> {
    let single = |ty: ValueType| Ports {
        inputs: Vec::new(),
        outputs: vec![out("value", PortType::Value(ty)).port()],
    };
    let specs = |inputs: &[PortSpec], outputs: &[PortSpec]| Ports {
        inputs: inputs.iter().map(PortSpec::port).collect(),
        outputs: outputs.iter().map(PortSpec::port).collect(),
    };
    let hosted = |kinds: &[GraphKind]| match kinds.contains(&graph.kind()) {
        true => Ok(()),
        false => Err(Problem::WrongGraph(graph.kind())),
    };
    Ok(match kind {
        NodeKind::Op(op) => {
            let info = op.info();
            specs(&info.inputs, &info.outputs)
        }
        NodeKind::Constant(constant) => {
            if !constant.is_finite() {
                return Err(Problem::NotFinite);
            }
            single(constant.ty())
        }
        NodeKind::Parameter(name) => single(
            graph
                .parameter(name)
                .ok_or_else(|| Problem::UnknownParameter(name.clone()))?
                .ty(),
        ),
        NodeKind::Input(input) => single(input.ty()),
        NodeKind::Swizzle(mask) => {
            let width = swizzle(mask)
                .ok_or_else(|| Problem::InvalidSwizzle(mask.clone()))?
                .len();
            specs(
                &[PortSpec::new(
                    "value",
                    PortType::Dynamic,
                    PortDefault::Value(Constant::vec4(0.0, 0.0, 0.0, 0.0)),
                )],
                &[out(
                    "value",
                    PortType::Value(ValueType::vector(width).expect("a swizzle width")),
                )],
            )
        }
        NodeKind::Texture(kind) => {
            hosted(&[GraphKind::Material, GraphKind::Composite])?;
            let texture = PortSpec::empty("texture", ValueType::Texture);
            let uv = context("uv", ValueType::Float2, Input::Uv);
            match kind {
                TextureKind::Colour | TextureKind::Data => specs(&[texture, uv], RGBA),
                TextureKind::Ramp => specs(
                    &[
                        texture,
                        value("position", ValueType::Float, Constant::scalar(0.5)),
                    ],
                    RGBA,
                ),
                TextureKind::Normal => specs(
                    &[texture, uv],
                    &[out("normal", PortType::Value(ValueType::Float3))],
                ),
            }
        }
        NodeKind::Fbm(_) => specs(
            &[
                POSITION,
                value("scale", ValueType::Float, Constant::scalar(1.0)),
                value("octaves", ValueType::Float, Constant::scalar(4.0)).hint(Hint::Integer),
                value("lacunarity", ValueType::Float, Constant::scalar(2.0)),
                value("gain", ValueType::Float, Constant::scalar(0.5)),
            ],
            &[out("value", PortType::Value(ValueType::Float))],
        ),
        NodeKind::Blend(_) => specs(
            &[
                PortSpec::color("base", 0.5, 0.5, 0.5),
                PortSpec::color("blend", 0.5, 0.5, 0.5),
                PortSpec::number("opacity", 1.0).range(0.0, 1.0),
            ],
            &[out("value", PortType::Value(ValueType::Float3))],
        ),
        NodeKind::ColorRamp(ramp) => {
            let finite = ramp.stops.iter().all(|stop| {
                stop.position.is_finite() && stop.color.iter().all(|value| value.is_finite())
            });
            if ramp.stops.is_empty() || !finite {
                return Err(Problem::EmptyRamp);
            }
            specs(
                &[value("position", ValueType::Float, Constant::scalar(0.5))],
                &[out("value", PortType::Value(ValueType::Float4))],
            )
        }
        NodeKind::Curve(curve) => {
            let finite = curve
                .points
                .iter()
                .all(|point| point.iter().all(|value| value.is_finite()));
            if curve.points.is_empty() || !finite {
                return Err(Problem::EmptyRamp);
            }
            specs(
                &[value("value", ValueType::Float, Constant::scalar(0.5))],
                &[out("value", PortType::Value(ValueType::Float))],
            )
        }
        NodeKind::Lighting => {
            hosted(&[GraphKind::Material])?;
            specs(
                &[
                    PortSpec::vector("light", 0.0, 0.0, 0.0),
                    PortSpec::vector("ambient", 0.0, 0.0, 0.0),
                ],
                SHADER,
            )
        }
        NodeKind::Emission => {
            hosted(&[GraphKind::Material])?;
            specs(
                &[
                    PortSpec::color("color", 1.0, 1.0, 1.0),
                    PortSpec::number("strength", 1.0).soft(0.0, 10.0),
                ],
                SHADER,
            )
        }
        NodeKind::Transparent => {
            hosted(&[GraphKind::Material])?;
            specs(&[], SHADER)
        }
        NodeKind::MixShader => {
            hosted(&[GraphKind::Material])?;
            specs(
                &[
                    PortSpec::empty("a", ValueType::Shader),
                    PortSpec::empty("b", ValueType::Shader),
                    PortSpec::number("factor", 0.5).range(0.0, 1.0),
                ],
                SHADER,
            )
        }
        NodeKind::AddShader => {
            hosted(&[GraphKind::Material])?;
            specs(
                &[
                    PortSpec::empty("a", ValueType::Shader),
                    PortSpec::empty("b", ValueType::Shader),
                ],
                SHADER,
            )
        }
        NodeKind::ShaderToRgb => {
            hosted(&[GraphKind::Material])?;
            specs(
                &[PortSpec::empty("shader", ValueType::Shader)],
                &[
                    PortSpec::output("color", ValueType::Float3),
                    PortSpec::output("alpha", ValueType::Float),
                ],
            )
        }
        NodeKind::Layer(_) => {
            hosted(&[GraphKind::Composite])?;
            specs(&[], &[PortSpec::output("image", ValueType::Image)])
        }
        NodeKind::Sample => {
            hosted(&[GraphKind::Composite])?;
            specs(
                &[
                    PortSpec::required("image", ValueType::Image),
                    context("uv", ValueType::Float2, Input::Uv),
                ],
                RGBA,
            )
        }
        NodeKind::Extern(id) => {
            let spec = library
                .get(id)
                .ok_or_else(|| Problem::UnknownKind(id.clone()))?;
            if !spec.hosts.holds(graph.kind()) {
                return Err(Problem::WrongGraph(graph.kind()));
            }
            specs(&spec.inputs, &spec.outputs)
        }
        NodeKind::Call(name) => {
            let function = graph
                .function(name)
                .ok_or_else(|| Problem::UnknownFunction(name.clone()))?;
            Ports {
                inputs: function
                    .inputs
                    .iter()
                    .map(|port| Port {
                        name: port.name.clone(),
                        ty: PortType::Value(port.ty),
                        default: port.default.map_or(
                            match port.ty.is_number() {
                                true => PortDefault::Required,
                                false => PortDefault::Empty,
                            },
                            PortDefault::Value,
                        ),
                        hint: Hint::None,
                        many: false,
                    })
                    .collect(),
                outputs: function
                    .outputs
                    .iter()
                    .map(|port| out_port(&port.name, port.ty))
                    .collect(),
            }
        }
        NodeKind::FunctionInput(name) => {
            let port = function_port(graph, scope, name, false)?;
            Ports {
                inputs: Vec::new(),
                outputs: vec![out_port("value", port.ty)],
            }
        }
        NodeKind::FunctionOutput(name) => {
            let port = function_port(graph, scope, name, true)?;
            Ports {
                inputs: vec![Port {
                    name: "value".to_owned(),
                    ty: PortType::Value(port.ty),
                    default: port
                        .default
                        .map_or(PortDefault::Required, PortDefault::Value),
                    hint: Hint::None,
                    many: false,
                }],
                outputs: Vec::new(),
            }
        }
        NodeKind::Output => {
            if *scope != Scope::Main {
                return Err(Problem::OutputInFunction);
            }
            Ports {
                inputs: output_ports(graph.kind(), library)
                    .iter()
                    .map(PortSpec::port)
                    .collect(),
                outputs: Vec::new(),
            }
        }
    })
}

fn out_port(name: &str, ty: ValueType) -> Port {
    Port {
        name: name.to_owned(),
        ty: PortType::Value(ty),
        default: PortDefault::Required,
        hint: Hint::None,
        many: false,
    }
}

fn function_port<'a>(
    graph: &'a Graph,
    scope: &Scope,
    name: &str,
    output: bool,
) -> Result<&'a crate::graph::FunctionPort, Problem> {
    let Scope::Function(function) = scope else {
        return Err(Problem::OutsideFunction);
    };
    let function = graph
        .function(function)
        .ok_or_else(|| Problem::UnknownFunction(function.clone()))?;
    let ports = if output {
        &function.outputs
    } else {
        &function.inputs
    };
    ports
        .iter()
        .find(|port| port.name == name)
        .ok_or_else(|| Problem::UnknownFunctionPort(name.to_owned()))
}

pub(crate) fn swizzle(mask: &str) -> Option<Vec<usize>> {
    if mask.is_empty() || mask.len() > 4 {
        return None;
    }
    mask.chars()
        .map(|component| match component {
            'x' | 'r' => Some(0),
            'y' | 'g' => Some(1),
            'z' | 'b' => Some(2),
            'w' | 'a' => Some(3),
            _ => None,
        })
        .collect()
}
