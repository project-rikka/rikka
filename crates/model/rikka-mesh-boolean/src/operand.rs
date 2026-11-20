use glam::Mat4;
use rikka_polymesh::Polymesh;

#[derive(Clone, Debug)]
pub struct Operand<'a> {
    pub(crate) mesh: &'a Polymesh,
    pub(crate) transform: Mat4,
    pub(crate) material_remap: Option<Vec<i32>>,
}

impl<'a> Operand<'a> {
    pub fn new(mesh: &'a Polymesh) -> Operand<'a> {
        Operand {
            mesh,
            transform: Mat4::IDENTITY,
            material_remap: None,
        }
    }

    #[must_use]
    pub fn transform(mut self, transform: Mat4) -> Operand<'a> {
        self.transform = transform;
        self
    }

    #[must_use]
    pub fn material_remap(mut self, remap: impl Into<Vec<i32>>) -> Operand<'a> {
        self.material_remap = Some(remap.into());
        self
    }
}
