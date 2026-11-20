use rikka_compositor_types::{Flip, Format};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout<S> {
    pub surface: S,
    pub format: Format,
    pub flip: Flip,
}
