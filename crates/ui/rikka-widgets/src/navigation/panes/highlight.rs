use rikka_types::{Point, Rect};
use rikka_ui::{Corner, Layout, Length, PaintCtx, Quad, Widget, motion};

use crate::navigation::panes::{DropTarget, region};

const TINT: f32 = 0.35;

#[derive(Widget)]
pub(super) struct Highlight {
    #[debug]
    landing: DropTarget,
}

pub(super) fn highlight(landing: DropTarget) -> Highlight {
    Highlight { landing }
}

impl Widget for Highlight {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::FILL)
            .placed_at(Point::ZERO)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let shown = region(
            Rect::from_origin_size(Point::ZERO, bounds.size),
            self.landing,
        );
        let rect = ctx
            .animate_curve("shape", shown, motion::INDICATOR)
            .translate(bounds.origin);
        let grown = ctx.play("grown", motion::HOVER);
        let selection = ctx.palette().selection;
        ctx.with_opacity(grown, |ctx| {
            ctx.draw_quad(
                Quad::new(rect)
                    .fill(selection.scale_alpha(TINT))
                    .radius(Corner::Item),
            );
        });
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }
}
