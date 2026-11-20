use rikka_text::IconSpec;
use rikka_types::{Point, Rect, Size};
use rikka_ui::{Element, Layout, Length, MaterialSymbol, PaintCtx, View, Widget, palette};

use crate::cluster::SYMBOL;

const MARK: f32 = 3.5;

pub(crate) struct Bell {
    quiet: bool,
    unread: bool,
}

impl Bell {
    #[must_use]
    pub(crate) fn new(quiet: bool, unread: bool) -> Option<Bell> {
        (quiet || unread).then_some(Bell { quiet, unread })
    }

    fn symbol(&self) -> MaterialSymbol {
        match self.quiet {
            true => MaterialSymbol::NOTIFICATIONS_OFF,
            false => MaterialSymbol::NOTIFICATIONS,
        }
    }
}

impl Widget for Bell {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::px(SYMBOL), Length::px(SYMBOL))
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
        if self.unread {
            ctx.draw_circle(
                Point::new(bounds.max_x(), bounds.y() + MARK),
                MARK,
                palette.accent,
            );
        }
    }

    fn debug_name(&self) -> &'static str {
        "TaskbarBell"
    }
}

impl std::fmt::Debug for Bell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bell")
            .field("quiet", &self.quiet)
            .field("unread", &self.unread)
            .finish()
    }
}

impl View for Bell {
    fn build(self) -> Element {
        Element::new(self)
    }
}
