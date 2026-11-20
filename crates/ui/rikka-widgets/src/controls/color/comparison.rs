use rikka_types::{Color, CornerRadii, PointerButton, Rect, RoundedRect};
use rikka_ui::{
    Action, Behavior, CursorShape, Event, EventCtx, Layout, Length, PaintCtx, Quad, Widget,
};

use crate::control::ControlSize;
use crate::controls::color::{paint_swatch, same_color};

const WIDTH: f32 = 1.6;

#[derive(Widget)]
pub(crate) struct Comparison {
    #[debug]
    pub(crate) old: Color,
    #[debug]
    pub(crate) new: Color,
    pub(crate) size: ControlSize,
    pub(crate) on_revert: Action,
}

impl Comparison {
    fn halves(bounds: Rect) -> (Rect, Rect) {
        let half = bounds.width() / 2.0;
        (
            Rect::new(bounds.x(), bounds.y(), half, bounds.height()),
            Rect::new(bounds.x() + half, bounds.y(), half, bounds.height()),
        )
    }

    fn reverts(&self) -> bool {
        self.on_revert.is_set() && !same_color(self.old, self.new)
    }
}

impl Widget for Comparison {
    type State = ();

    fn layout(&self) -> Layout {
        let height = self.size.height();
        Layout::row()
            .size(Length::px((height * WIDTH).round()), Length::px(height))
            .fixed()
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let radius = self.size.radius();
        let outline = ctx.theme().palette().outline;
        let (old, new) = Self::halves(bounds);
        let left = CornerRadii {
            top_left: radius,
            bottom_left: radius,
            ..CornerRadii::ZERO
        };
        let right = CornerRadii {
            top_right: radius,
            bottom_right: radius,
            ..CornerRadii::ZERO
        };
        paint_swatch(ctx, RoundedRect::new(old, left), self.old);
        paint_swatch(ctx, RoundedRect::new(new, right), self.new);
        ctx.draw_quad(Quad::new(bounds).stroke(1.0, outline).radius(radius));
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerPressed {
            button: PointerButton::Primary,
            position,
            ..
        } = *event
            && self.reverts()
            && Self::halves(ctx.bounds()).0.contains(position)
            && let Some(on_revert) = self.on_revert.handler()
        {
            ctx.set_handled();
            ctx.call(on_revert, ());
        }
    }

    fn cursor(
        &self,
        _state: &Self::State,
        bounds: Rect,
        point: rikka_types::Point,
    ) -> Option<CursorShape> {
        (self.reverts() && Self::halves(bounds).0.contains(point)).then_some(CursorShape::Pointer)
    }
}
