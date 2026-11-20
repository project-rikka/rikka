#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Domain {
    Point,
    Edge,
    Face,
    Corner,
}

impl Domain {
    pub const ALL: [Domain; 4] = [Domain::Point, Domain::Edge, Domain::Face, Domain::Corner];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Domain::Point => "point",
            Domain::Edge => "edge",
            Domain::Face => "face",
            Domain::Corner => "corner",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DomainSizes {
    pub points: usize,
    pub edges: usize,
    pub faces: usize,
    pub corners: usize,
}

impl DomainSizes {
    #[must_use]
    pub fn get(&self, domain: Domain) -> usize {
        match domain {
            Domain::Point => self.points,
            Domain::Edge => self.edges,
            Domain::Face => self.faces,
            Domain::Corner => self.corners,
        }
    }
}
