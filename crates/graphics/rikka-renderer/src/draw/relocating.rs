use crate::draw::emit::whole_shift;
use crate::draw::retained::RetainedBatch;
use crate::draw::shown::VisibilityTable;
use crate::scene::{Node, NodeId, Shape};

impl RetainedBatch {
    pub(crate) fn relocate(&mut self, id: NodeId, node: &Node) -> bool {
        let Some(slot) = self.tracked_slot(id) else {
            return false;
        };
        let (span, at, band) = (slot.span, slot.at, slot.band);
        if span.count == 0 {
            return false;
        }
        let shift = match &node.shape {
            Shape::Text(_) => {
                let Some(across) = whole_shift(node, band, at.x) else {
                    return false;
                };
                let down = node.rect.min_y().round() - at.y.round();
                [across, down]
            }
            Shape::Icon(_) => return false,
            Shape::Rect
            | Shape::RoundedRect { .. }
            | Shape::Circle
            | Shape::Arc { .. }
            | Shape::Capsule { .. }
            | Shape::Image { .. } => [node.rect.min_x() - at.x, node.rect.min_y() - at.y],
        };
        self.moves.set(VisibilityTable::place(id.index), shift);
        true
    }
}
