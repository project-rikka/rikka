use crate::pixmap::Pixmap;
use crate::transform::Transform;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tiling {
    #[default]
    Once,
    Repeat,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Sampling {
    Nearest,
    #[default]
    Bilinear,
}

#[derive(Clone, Copy, Debug)]
pub struct Pattern<'a> {
    pub pixmap: &'a Pixmap,
    pub transform: Transform,
    pub tiling: Tiling,
    pub sampling: Sampling,
    pub opacity: f32,
}
