mod desc;
pub(crate) mod features;
mod overrides;
pub(crate) mod pool;
mod progress;
pub(crate) mod roles;
mod state;
pub(crate) mod template;
pub(crate) mod value;

pub use desc::MaterialDesc;
pub use overrides::{Override, Overrides, Scope};
pub use progress::TemplateReadiness;
pub use rikka_shader_programs::{BuiltinTemplate, TemplateArtifact};
pub use state::{Cull, MaterialPass, MaterialState, Polygon};
pub use template::MaterialTemplate;
pub use value::{TextureBinding, Value};

pub(crate) use pool::MaterialPool;
pub(crate) use template::Program;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}
