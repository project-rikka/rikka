use crate::clip::ClipId;
use crate::project::Edge;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditPoint {
    pub clip: ClipId,
    pub edge: Edge,
}

impl EditPoint {
    #[must_use]
    pub fn head(clip: ClipId) -> EditPoint {
        EditPoint {
            clip,
            edge: Edge::Head,
        }
    }

    #[must_use]
    pub fn tail(clip: ClipId) -> EditPoint {
        EditPoint {
            clip,
            edge: Edge::Tail,
        }
    }
}
