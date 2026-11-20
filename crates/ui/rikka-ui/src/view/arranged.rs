use rikka_types::Rect;

#[derive(Clone, Copy, Debug)]
pub struct Arranged<'a> {
    room: Rect,
    children: &'a [Rect],
    anchor: Option<ScrollAnchor>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAnchor {
    pub was: f32,
    pub is: f32,
    pub touched: bool,
}

impl ScrollAnchor {
    pub fn moved(&self) -> f32 {
        self.is - self.was
    }
}

impl<'a> Arranged<'a> {
    pub(crate) fn new(room: Rect, children: &'a [Rect], anchor: Option<ScrollAnchor>) -> Self {
        Self {
            room,
            children,
            anchor,
        }
    }

    pub fn room(&self) -> Rect {
        self.room
    }

    pub fn child_rect(&self, index: usize) -> Option<Rect> {
        self.children.get(index).copied()
    }

    pub fn anchor(&self) -> Option<ScrollAnchor> {
        self.anchor
    }
}
