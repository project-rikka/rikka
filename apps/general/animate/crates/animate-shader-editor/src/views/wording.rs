use animate_shader::Status;
use rikka_app::prelude::*;
use rikka_renderer_3d::{Hint, Role, TextureKind};
use rikka_shader_graph::{Category, Interpolation, Noise, ValueType};

use crate::state::Tab;
use crate::views::parameters::Shape;

pub(crate) fn tab(tab: Tab) -> Label {
    match tab {
        Tab::World => tr!("animate.shader.tab.world"),
        Tab::Composite => tr!("animate.shader.tab.composite"),
        Tab::Material => tr!("animate.shader.tab.material"),
    }
}

pub(crate) fn class(class: &str) -> Label {
    tr!(&format!("animate.shader.class.{class}"))
}

pub(crate) fn status(status: &Status) -> (Label, Color) {
    let palette = palette();
    match status {
        Status::Compiling => (tr!("animate.shader.compiling"), palette.text_muted),
        Status::Ready => (tr!("animate.shader.ready"), palette.success),
        Status::Failed(diagnostics) => (
            tr!("animate.shader.failed", count = diagnostics.len().max(1)),
            palette.danger,
        ),
    }
}

pub(crate) fn category(category: Category) -> Label {
    tr!(&format!(
        "animate.shader.category.{}",
        match category {
            Category::Math => "math",
            Category::Interpolation => "interpolation",
            Category::Trigonometry => "trigonometry",
            Category::Vector => "vector",
            Category::Uv => "uv",
            Category::Noise => "noise",
            Category::Surface => "surface",
            Category::Color => "color",
            Category::Ramp => "ramp",
            Category::Texture => "texture",
            Category::Lighting => "lighting",
            Category::Shader => "shader",
            Category::Toon => "toon",
            Category::Layer => "layer",
            Category::Filter => "filter",
            Category::Lens => "lens",
            Category::Stylize => "stylize",
            Category::Environment => "environment",
            Category::Light => "light",
            Category::Render => "render",
            Category::Stage => "stage",
            Category::Input => "input",
            Category::Parameter => "parameter",
            Category::Constant => "constant",
            Category::Function => "function",
            Category::Output => "output",
        }
    ))
}

pub(crate) fn value_type(ty: ValueType) -> String {
    match ty {
        ValueType::Float => tr!("animate.shader.type.float").to_string(),
        ValueType::Float2 => tr!("animate.shader.type.float2").to_string(),
        ValueType::Float3 => tr!("animate.shader.type.float3").to_string(),
        ValueType::Float4 => tr!("animate.shader.type.float4").to_string(),
        ValueType::Texture => tr!("animate.shader.type.texture").to_string(),
        ValueType::Shader => tr!("animate.shader.type.shader").to_string(),
        ValueType::Image => tr!("animate.shader.type.image").to_string(),
        ValueType::Record(kind) => kind.to_string(),
    }
}

pub(crate) fn texture_kind(kind: TextureKind) -> String {
    match kind {
        TextureKind::Colour => tr!("animate.shader.texture_kinds.colour"),
        TextureKind::Data => tr!("animate.shader.texture_kinds.data"),
        TextureKind::Normal => tr!("animate.shader.texture_kinds.normal"),
        TextureKind::Ramp => tr!("animate.shader.texture_kinds.ramp"),
    }
    .to_string()
}

pub(crate) fn noise(noise: Noise) -> String {
    match noise {
        Noise::Value => tr!("animate.shader.noise_value"),
        Noise::Gradient => tr!("animate.shader.noise_gradient"),
    }
    .to_string()
}

pub(crate) fn interpolation(interpolation: Interpolation) -> Label {
    match interpolation {
        Interpolation::Linear => tr!("animate.shader.interpolation_linear"),
        Interpolation::Smooth => tr!("animate.shader.interpolation_smooth"),
        Interpolation::Constant => tr!("animate.shader.interpolation_constant"),
    }
}

pub(crate) fn parameter_shape(shape: Shape) -> Label {
    match shape {
        Shape::Float => tr!("animate.shader.shape.float"),
        Shape::Vec2 => tr!("animate.shader.shape.vec2"),
        Shape::Vec3 => tr!("animate.shader.shape.vec3"),
        Shape::Vec4 => tr!("animate.shader.shape.vec4"),
        Shape::Color => tr!("animate.shader.shape.color"),
        Shape::ColorAlpha => tr!("animate.shader.shape.color_alpha"),
        Shape::Int => tr!("animate.shader.shape.int"),
        Shape::UInt => tr!("animate.shader.shape.uint"),
        Shape::Bool => tr!("animate.shader.shape.bool"),
        Shape::Enum => tr!("animate.shader.shape.enum"),
        Shape::Texture => tr!("animate.shader.shape.texture"),
    }
}

pub(crate) fn role(role: Option<Role>) -> Label {
    match role {
        None => tr!("animate.shader.role_none"),
        Some(role) => tr!(&format!("animate.shader.roles.{}", role.name())),
    }
}

pub(crate) fn hint(hint: Hint) -> Label {
    match hint {
        Hint::Angle => tr!("animate.shader.hints.angle"),
        Hint::Logarithmic => tr!("animate.shader.hints.logarithmic"),
        Hint::Hidden => tr!("animate.shader.hints.hidden"),
        Hint::Advanced => tr!("animate.shader.hints.advanced"),
    }
}
