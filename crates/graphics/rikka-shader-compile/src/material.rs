use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{MaterialTemplate, ShaderCompiler, TextureBinding, Value};
use rikka_shader_graph::{
    Constant, Graph, Library, MaterialSource, Socket, SocketRef, SocketValue, ValueType,
};

use crate::error::Error;

#[derive(Clone, Debug)]
pub struct Material {
    source: MaterialSource,
    template: MaterialTemplate,
}

pub fn socket_value(ty: ValueType, constant: Constant) -> Option<Value> {
    let at = |index: usize| constant.resized(4).component(index);
    Some(match ty {
        ValueType::Float => Value::Float(at(0)),
        ValueType::Float2 => Value::Vec2(Vec2::new(at(0), at(1))),
        ValueType::Float3 => Value::Vec3(Vec3::new(at(0), at(1), at(2))),
        ValueType::Float4 => Value::Vec4(Vec4::new(at(0), at(1), at(2), at(3))),
        _ => return None,
    })
}

impl Material {
    pub fn compile(
        graph: &Graph,
        library: &Library,
        compiler: &ShaderCompiler,
        name: &str,
    ) -> Result<Self, Error> {
        let source = graph.material(library, name)?;
        let template = compiler.template(&source.name, &source.source)?;
        Ok(Self { source, template })
    }

    pub fn reload(
        &mut self,
        graph: &Graph,
        library: &Library,
        compiler: &ShaderCompiler,
    ) -> Result<(), Error> {
        let source = graph.material(library, &self.source.name)?;
        if source.source != self.source.source {
            self.template.reload(compiler, &source.source)?;
        }
        self.source = source;
        Ok(())
    }

    pub fn template(&self) -> &MaterialTemplate {
        &self.template
    }

    pub fn source(&self) -> &MaterialSource {
        &self.source
    }

    pub fn sockets(&self) -> &[Socket] {
        &self.source.sockets
    }

    pub fn values<'a>(
        &'a self,
        held: impl Fn(&SocketRef) -> Option<SocketValue> + 'a,
        texture: impl Fn(&str) -> Option<TextureBinding> + 'a,
    ) -> impl Iterator<Item = (&'a str, Value)> + 'a {
        self.source.sockets.iter().filter_map(move |socket| {
            let value = match held(&socket.at)? {
                SocketValue::Number(constant) => socket_value(socket.ty, constant)?,
                SocketValue::Asset(path) => Value::Texture(texture(&path)),
            };
            Some((socket.name.as_str(), value))
        })
    }
}
