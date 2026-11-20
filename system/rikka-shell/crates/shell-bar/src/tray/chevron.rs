use rikka_text::IconSpec;
use rikka_types::{Point, Rect, Size};
use rikka_ui::{Element, Layout, Length, MaterialSymbol, PaintCtx, View, Widget, palette};

use crate::cluster::{DEPTH, INSET, SYMBOL};
use crate::tray::MARK;

#[derive(Debug)]
pub(super) struct Chevron {
    unfolded: bool,
    attention: bool,
}

impl Chevron {
    #[must_use]
    pub(crate) fn new(unfolded: bool, attention: bool) -> Chevron {
        Chevron {
            unfolded,
            attention,
        }
    }

    fn symbol(&self) -> MaterialSymbol {
        match self.unfolded {
            true => MaterialSymbol::EXPAND_LESS,
            false => MaterialSymbol::EXPAND_MORE,
        }
    }
}

impl Widget for Chevron {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(
                Length::px(SYMBOL + INSET * 2.0),
                Length::px(SYMBOL + DEPTH * 2.0),
            )
            .fixed()
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let palette = palette();
        ctx.draw_icon(
            Rect::from_center(bounds.center(), Size::new(SYMBOL, SYMBOL)),
            IconSpec::new(self.symbol().codepoint(), SYMBOL),
            palette.text,
        );
        if self.attention {
            let corner = Point::new(bounds.max_x() - INSET / 2.0, bounds.y() + DEPTH);
            ctx.draw_circle(corner, MARK, palette.accent);
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.unfolded == other.unfolded && self.attention == other.attention
    }

    fn debug_name(&self) -> &'static str {
        "TrayChevron"
    }
}

impl View for Chevron {
    fn build(self) -> Element {
        Element::new(self)
    }
}
