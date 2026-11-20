use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rikka_types::Rect;
use rikka_ui::{PaintCtx, Watchers};

#[derive(Clone, Debug, Default)]
pub(crate) struct Cards(Rc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    laid: RefCell<HashMap<usize, Card>>,
    resting: Watchers,
}

#[derive(Clone, Copy, Debug)]
struct Card {
    rect: Rect,
    followed: Option<Rect>,
}

impl Cards {
    pub(super) fn lay(&self, cards: impl IntoIterator<Item = (usize, Rect)>) {
        let mut moved = false;
        {
            let mut laid = self.0.laid.borrow_mut();
            let mut fresh = HashMap::with_capacity(laid.len());
            for (number, rect) in cards {
                let card = match laid.get(&number) {
                    Some(card) => {
                        moved |= card.rect != rect;
                        Card { rect, ..*card }
                    }
                    None => {
                        moved = true;
                        Card {
                            rect,
                            followed: None,
                        }
                    }
                };
                fresh.insert(number, card);
            }
            *laid = fresh;
        }
        if moved {
            self.0.resting.repaint();
        }
    }

    pub(crate) fn follow(&self, number: usize, standing: Rect, ctx: &mut PaintCtx<'_>) -> Rect {
        let mut laid = self.0.laid.borrow_mut();
        let Some(card) = laid.get_mut(&number) else {
            self.0.resting.watch(ctx.handle());
            return standing;
        };
        match card.followed.replace(card.rect) == Some(card.rect) {
            true => self.0.resting.watch(ctx.handle()),
            false => ctx.request_frame(),
        }
        card.rect
    }
}
