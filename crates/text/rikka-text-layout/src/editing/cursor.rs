#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Affinity {
    Before,
    #[default]
    After,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cursor {
    pub index: usize,
    pub affinity: Affinity,
}

impl Cursor {
    pub const fn new(index: usize, affinity: Affinity) -> Self {
        Self { index, affinity }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Movement {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caret {
    pub x: f32,
    pub row: usize,
    pub top: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionRect {
    pub row: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
