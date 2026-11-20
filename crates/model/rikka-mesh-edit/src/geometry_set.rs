use crate::ids::{EdgeId, Element, FaceId, VertId};
use crate::sets::IdSet;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Geometry {
    pub verts: IdSet<VertId>,
    pub edges: IdSet<EdgeId>,
    pub faces: IdSet<FaceId>,
}

impl Geometry {
    #[must_use]
    pub fn new() -> Geometry {
        Geometry::default()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verts.is_empty() && self.edges.is_empty() && self.faces.is_empty()
    }

    pub fn insert(&mut self, element: Element) -> bool {
        match element {
            Element::Vert(vert) => self.verts.insert(vert),
            Element::Edge(edge) => self.edges.insert(edge),
            Element::Face(face) => self.faces.insert(face),
        }
    }

    #[must_use]
    pub fn contains(&self, element: Element) -> bool {
        match element {
            Element::Vert(vert) => self.verts.contains(vert),
            Element::Edge(edge) => self.edges.contains(edge),
            Element::Face(face) => self.faces.contains(face),
        }
    }

    pub fn clear(&mut self) {
        self.verts.clear();
        self.edges.clear();
        self.faces.clear();
    }
}

impl FromIterator<Element> for Geometry {
    fn from_iter<T: IntoIterator<Item = Element>>(elements: T) -> Geometry {
        let mut geometry = Geometry::new();
        for element in elements {
            geometry.insert(element);
        }
        geometry
    }
}
