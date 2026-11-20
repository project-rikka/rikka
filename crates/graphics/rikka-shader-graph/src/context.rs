use crate::node::{DirectionSpace, Input, PositionSpace, ViewSpace};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Site {
    Vertex,
    Hull,
    Surface,
    Coverage,
    Light,
    Indirect,
    Post,
    Pixel,
    Uniform,
}

impl Site {
    pub(crate) fn pixel(self) -> Option<&'static str> {
        match self {
            Self::Surface => Some("pixel"),
            Self::Light | Self::Indirect | Self::Post => Some("state.pixel"),
            _ => None,
        }
    }

    pub(crate) fn is_material(self) -> bool {
        !matches!(self, Self::Pixel | Self::Uniform)
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Vertex => "displacement",
            Self::Hull => "outline",
            Self::Surface => "surface",
            Self::Coverage => "coverage",
            Self::Light => "light",
            Self::Indirect => "ambient",
            Self::Post => "combine",
            Self::Pixel => "pixel",
            Self::Uniform => "value",
        }
    }

    pub(crate) fn token(self, token: &str) -> Option<&'static str> {
        match (token, self) {
            ("pixel", _) => self.pixel(),
            ("context", site) if site.is_material() => Some("context"),
            ("light", Self::Light) => Some("light"),
            ("shadow", Self::Light) => Some("shadow"),
            ("indirect", Self::Indirect) => Some("indirect"),
            ("vertex", Self::Vertex | Self::Hull) => Some("vertex"),
            ("scale", Self::Hull) => Some("scale"),
            ("uv", Self::Pixel) => Some("uv"),
            ("texel", Self::Pixel) => Some("pixel"),
            ("size", Self::Pixel) => Some("push.size"),
            _ => None,
        }
    }
}

pub(crate) const TOKENS: [&str; 10] = [
    "pixel", "context", "light", "shadow", "indirect", "vertex", "scale", "uv", "texel", "size",
];

fn view_space(direction: String) -> String {
    format!("mul((float3x3)context.view.view, {direction})")
}

pub(crate) fn input_code(input: Input, site: Site) -> Option<String> {
    let pixel = site.pixel();
    let vertex = matches!(site, Site::Vertex | Site::Hull);
    let world = pixel
        .map(|pixel| format!("{pixel}.world_position"))
        .or_else(|| vertex.then(|| "vertex.world_position".to_owned()));
    let normal = pixel
        .map(|pixel| format!("{pixel}.geometric_normal"))
        .or_else(|| vertex.then(|| "vertex.normal".to_owned()));
    let tangent = pixel
        .map(|pixel| format!("{pixel}.tangent.xyz"))
        .or_else(|| vertex.then(|| "vertex.tangent.xyz".to_owned()));
    let view = pixel.map(|pixel| format!("{pixel}.view")).or_else(|| {
        vertex.then(|| "normalize(context.view.camera_position - vertex.world_position)".to_owned())
    });
    let field = |name: &str| pixel.map(|pixel| format!("{pixel}.{name}"));
    match input {
        Input::Position(PositionSpace::World) => world,
        Input::Position(PositionSpace::Object) => {
            (site == Site::Vertex).then(|| "vertex.local_position".to_owned())
        }
        Input::Position(PositionSpace::View) => {
            world.map(|world| format!("mul(context.view.view, float4({world}, 1.0)).xyz"))
        }
        Input::Normal(DirectionSpace::World) => normal,
        Input::Normal(DirectionSpace::View) => normal.map(view_space),
        Input::Tangent(DirectionSpace::World) => tangent,
        Input::Tangent(DirectionSpace::View) => tangent.map(view_space),
        Input::ViewDirection(ViewSpace::World) => view,
        Input::ViewDirection(ViewSpace::View) => view.map(view_space),
        Input::ViewDirection(ViewSpace::Tangent) => pixel.map(|pixel| {
            format!(
                "rikka_graph_to_tangent({pixel}.view, {pixel}.geometric_normal, {pixel}.tangent)"
            )
        }),
        Input::Uv => match site {
            Site::Vertex | Site::Hull => Some("vertex.uv".to_owned()),
            Site::Coverage => Some("coverage.uv".to_owned()),
            Site::Pixel => Some("uv".to_owned()),
            _ => field("uv"),
        },
        Input::VertexColor => match site {
            Site::Vertex | Site::Hull => Some("vertex.color".to_owned()),
            _ => field("color"),
        },
        Input::Time => match site {
            Site::Vertex => Some("time".to_owned()),
            Site::Pixel => Some("push.time".to_owned()),
            Site::Uniform => None,
            _ => Some("context.time".to_owned()),
        },
        Input::ScreenPosition => match site {
            Site::Pixel => Some("(float2(pixel) + 0.5)".to_owned()),
            _ => field("frag_coord.xy"),
        },
        Input::ViewDepth => field("view_depth"),
        Input::VertexIndex => {
            (site == Site::Vertex).then(|| "float(vertex.vertex_index)".to_owned())
        }
        Input::FrontFacing => {
            pixel.map(|pixel| format!("({pixel}.front_facing && !{pixel}.hull ? 1.0 : 0.0)"))
        }
        Input::SmoothedNormal => (site == Site::Hull).then(|| "vertex.smoothed_normal".to_owned()),
        Input::LightDirection => (site == Site::Light).then(|| "light.direction".to_owned()),
        Input::LightColor => {
            (site == Site::Light).then(|| "(light.light.color * light.light.intensity)".to_owned())
        }
        Input::LightAttenuation => (site == Site::Light).then(|| "light.attenuation".to_owned()),
        Input::Shadow => (site == Site::Light).then(|| "shadow.visibility()".to_owned()),
        Input::Ambient => (site == Site::Indirect).then(|| "indirect.ambient".to_owned()),
        Input::AmbientOcclusion => {
            (site == Site::Indirect).then(|| "indirect.occlusion.x".to_owned())
        }
        Input::Size => (site == Site::Pixel).then(|| "float2(push.size)".to_owned()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Taint {
    Light,
    Indirect,
}

pub(crate) fn input_taint(input: Input) -> Option<Taint> {
    match input {
        Input::LightDirection | Input::LightColor | Input::LightAttenuation | Input::Shadow => {
            Some(Taint::Light)
        }
        Input::Ambient | Input::AmbientOcclusion => Some(Taint::Indirect),
        _ => None,
    }
}

pub(crate) fn code_taint(code: &str) -> Option<Taint> {
    if code.contains("$light") || code.contains("$shadow") {
        Some(Taint::Light)
    } else if code.contains("$indirect") {
        Some(Taint::Indirect)
    } else {
        None
    }
}

pub(crate) fn code_available(code: &str, site: Site) -> bool {
    TOKENS
        .iter()
        .all(|token| !code.contains(&format!("${token}")) || site.token(token).is_some())
}

pub(crate) fn expand(code: &str, site: Site, argument: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(at) = rest.find(['$', '{']) {
        out.push_str(&rest[..at]);
        let opener = rest.as_bytes()[at];
        let tail = &rest[at + 1..];
        if opener == b'$' {
            let length = tail
                .find(|letter: char| !letter.is_ascii_lowercase() && letter != '_')
                .unwrap_or(tail.len());
            let token = &tail[..length];
            match site.token(token) {
                Some(found) => out.push_str(found),
                None => {
                    out.push('$');
                    out.push_str(token);
                }
            }
            rest = &tail[length..];
        } else {
            let close = tail.find('}');
            let name = close.map(|close| &tail[..close]);
            match name.and_then(|name| argument(name).map(|text| (name, text))) {
                Some((name, text)) => {
                    out.push_str(&text);
                    rest = &tail[name.len() + 1..];
                }
                None => {
                    out.push('{');
                    rest = tail;
                }
            }
        }
    }
    out.push_str(rest);
    out
}
