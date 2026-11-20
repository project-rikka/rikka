use std::fmt::Debug;
use std::hash::Hash;

pub(crate) const NONE: u32 = u32::MAX;

pub trait ElementId: Copy + Eq + Ord + Hash + Debug + Send + Sync + 'static {
    fn from_index(index: usize) -> Self;
    fn index(self) -> usize;
}

macro_rules! element_id {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub struct $name(pub(crate) u32);

        impl $name {
            #[must_use]
            pub const fn new(index: u32) -> $name {
                $name(index)
            }

            #[must_use]
            pub const fn index(self) -> usize {
                self.0 as usize
            }

            #[must_use]
            pub const fn raw(self) -> u32 {
                self.0
            }
        }

        impl ElementId for $name {
            fn from_index(index: usize) -> $name {
                $name(index as u32)
            }

            fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

element_id!(VertId);
element_id!(EdgeId);
element_id!(LoopId);
element_id!(FaceId);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Element {
    Vert(VertId),
    Edge(EdgeId),
    Face(FaceId),
}

impl Element {
    #[must_use]
    pub fn kind(self) -> ElementKind {
        match self {
            Element::Vert(_) => ElementKind::Vert,
            Element::Edge(_) => ElementKind::Edge,
            Element::Face(_) => ElementKind::Face,
        }
    }
}

impl From<VertId> for Element {
    fn from(vert: VertId) -> Element {
        Element::Vert(vert)
    }
}

impl From<EdgeId> for Element {
    fn from(edge: EdgeId) -> Element {
        Element::Edge(edge)
    }
}

impl From<FaceId> for Element {
    fn from(face: FaceId) -> Element {
        Element::Face(face)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ElementKind {
    Vert,
    Edge,
    Face,
}
