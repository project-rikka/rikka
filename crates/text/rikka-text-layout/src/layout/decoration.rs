#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecorationKind {
    Underline,
    Strikethrough,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decoration {
    pub kind: DecorationKind,
    pub x: f32,
    pub width: f32,
    pub y: f32,
    pub thickness: f32,
    pub start: u32,
}
