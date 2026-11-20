use crate::rect::Rect;

pub(crate) const MAX_RECTS: usize = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Damage {
    rects: [Rect; MAX_RECTS],
    len: usize,
}

impl Damage {
    pub(crate) fn add(&mut self, rect: Rect) {
        if rect.is_empty() {
            return;
        }
        if self.len < MAX_RECTS {
            self.rects[self.len] = rect;
            self.len += 1;
            return;
        }
        self.rects[0] = self
            .rects
            .iter()
            .fold(rect, |bounds, other| bounds.union(other));
        self.len = 1;
    }

    pub(crate) fn rects(&self) -> &[Rect] {
        &self.rects[..self.len]
    }
}
