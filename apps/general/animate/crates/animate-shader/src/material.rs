use rikka_shader_graph::{Builder, Error, Graph, GraphKind, Library};

pub const PRINCIPLED: &str = "bsdf.principled";

pub fn empty_material(library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Material, library);
    let bsdf = builder.external(PRINCIPLED, []);
    let shader = builder.out(bsdf, "shader");
    builder.output([("surface", shader.into())]);
    builder.finish()
}
