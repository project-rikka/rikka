#[derive(Clone, Copy, Debug)]
pub(crate) struct ShapeRecord {
    pub body: u32,
    pub prev: u32,
    pub next: u32,
    pub density: f32,
}
