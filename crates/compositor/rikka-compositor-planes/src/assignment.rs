use crate::layout::Layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assignment<S> {
    Primary(Layout<S>),
    Composited(Option<Blocked<S>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocked<S> {
    pub surface: S,
    pub reason: Reason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reason {
    Format,
    Transform,
    Source,
    Size,
    Placement,
    Translucent,
    Covered,
    Refused,
    Pending,
}

impl<S: Copy> Assignment<S> {
    pub fn layout(&self) -> Option<Layout<S>> {
        match self {
            Self::Primary(layout) => Some(*layout),
            Self::Composited(_) => None,
        }
    }
}
