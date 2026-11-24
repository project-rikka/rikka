use rikka_settings::session::appearance::TaskbarPosition;
use rikka_text::IconSpec;
use rikka_types::{Point, Rect, Size};
use rikka_ui::{Element, Layout, Length, MaterialSymbol, PaintCtx, View, Widget, palette};
use shell_kit::taskbar::Edge;

use crate::axis;
use crate::cluster::{DEPTH, INSET, SYMBOL};
use crate::tray::MARK;

#[derive(Debug)]
pub(super) struct Chevron {
    unfolded: bool,
    attention: bool,
    edge: Edge,
}

impl Chevron {
    #[must_use]
    pub(crate) fn new(unfolded: bool, attention: bool, edge: Edge) -> Chevron {
        Chevron {
            unfolded,
            attention,
            edge,
        }
    }

    fn symbol(&self) -> MaterialSymbol {
        let (inward, outward) = match self.edge.position() {
            TaskbarPosition::Top => (MaterialSymbol::EXPAND_MORE, MaterialSymbol::EXPAND_LESS),
            TaskbarPosition::Bottom => (MaterialSymbol::EXPAND_LESS, MaterialSymbol::EXPAND_MORE),
            TaskbarPosition::Left => (MaterialSymbol::CHEVRON_RIGHT, MaterialSymbol::CHEVRON_LEFT),
            TaskbarPosition::Right => (MaterialSymbol::CHEVRON_LEFT, MaterialSymbol::CHEVRON_RIGHT),
        };
        match self.unfolded {
            true => outward,
            false => inward,
        }
    }
}

impl Widget for Chevron {
    type State = ();

    fn layout(&self) -> Layout {
        let size = axis::size(self.edge.axis(), SYMBOL + INSET * 2.0, SYMBOL + DEPTH * 2.0);
        Layout::row()
            .size(Length::px(size.width), Length::px(size.height))
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
            let inset = self.edge.axis().point(INSET / 2.0, DEPTH);
            let corner = Point::new(bounds.max_x() - inset.x, bounds.y() + inset.y);
            ctx.draw_circle(corner, MARK, palette.accent);
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.unfolded == other.unfolded
            && self.attention == other.attention
            && self.edge == other.edge
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
