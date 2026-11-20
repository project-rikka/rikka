use rikka_types::Rect;

use crate::runtime::LayoutNode;
use crate::scroll::Reveal;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Revealing {
    pub(crate) rect: Rect,
    pub(crate) reveal: Reveal,
    at: LayoutNode,
    with_parent_start: bool,
}

impl Revealing {
    pub(crate) fn new(rect: Rect, reveal: Reveal, at: LayoutNode) -> Revealing {
        Revealing {
            rect,
            reveal,
            at,
            with_parent_start: false,
        }
    }

    pub(crate) fn with_parent_start(self) -> Revealing {
        Revealing {
            with_parent_start: true,
            ..self
        }
    }

    pub(crate) fn reaches(&self, node: LayoutNode) -> bool {
        self.at == node
    }

    pub(crate) fn passed_to(self, parent: LayoutNode, bounds: Rect) -> Revealing {
        let rect = match self.with_parent_start {
            true => {
                let top = bounds.min_y().min(self.rect.min_y());
                Rect::new(
                    self.rect.x(),
                    top,
                    self.rect.width(),
                    self.rect.max_y() - top,
                )
            }
            false => self.rect,
        };
        Revealing {
            rect,
            reveal: self.reveal,
            at: parent,
            with_parent_start: false,
        }
    }
}
