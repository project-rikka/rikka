use rikka_physics_core::Executor;
use rikka_physics_math::Vec3;

#[must_use]
#[derive(Clone, Copy)]
pub struct MeshDef<'a> {
    pub(crate) vertices: &'a [Vec3],
    pub(crate) indices: &'a [u32],
    pub(crate) materials: &'a [u8],
    pub(crate) identify_edges: bool,
    pub(crate) clockwise: bool,
    pub(crate) executor: Option<&'a dyn Executor>,
}

impl std::fmt::Debug for MeshDef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshDef")
            .field("vertices", &self.vertices.len())
            .field("triangles", &(self.indices.len() / 3))
            .field("materials", &self.materials.len())
            .field("identify_edges", &self.identify_edges)
            .field("clockwise", &self.clockwise)
            .finish()
    }
}

impl<'a> MeshDef<'a> {
    pub fn new(vertices: &'a [Vec3], indices: &'a [u32]) -> Self {
        Self {
            vertices,
            indices,
            materials: &[],
            identify_edges: true,
            clockwise: false,
            executor: None,
        }
    }

    pub fn materials(mut self, materials: &'a [u8]) -> Self {
        self.materials = materials;
        self
    }

    pub fn identify_edges(mut self, identify: bool) -> Self {
        self.identify_edges = identify;
        self
    }

    pub fn clockwise(mut self, clockwise: bool) -> Self {
        self.clockwise = clockwise;
        self
    }

    pub fn executor(mut self, executor: &'a dyn Executor) -> Self {
        self.executor = Some(executor);
        self
    }
}
