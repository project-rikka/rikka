use crate::project::Edge;

use super::transition::Transition;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ends {
    pub into: Option<Transition>,
    pub out: Option<Transition>,
}

impl Ends {
    #[must_use]
    pub fn at(&self, edge: Edge) -> Option<&Transition> {
        match edge {
            Edge::Head => self.into.as_ref(),
            Edge::Tail => self.out.as_ref(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.into.is_none() && self.out.is_none()
    }

    pub(crate) fn set(&mut self, edge: Edge, transition: Option<Transition>) {
        match edge {
            Edge::Head => self.into = transition,
            Edge::Tail => self.out = transition,
        }
    }
}
