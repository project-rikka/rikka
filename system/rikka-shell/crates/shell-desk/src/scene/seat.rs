use desk_layout::WidgetId;
use rikka_types::{Point, Rect, Size};
use rikka_ui::{Element, Event, EventCtx, Layout, Length, PaintCtx, Widget};
use shell_kit::fit;

use crate::scene::touch::{Pressing, Touch};
use crate::scene::transition_style::TransitionStyle;

const PLACE: &str = "place";

const PRESENT: &str = "present";

const SHRUNK: f32 = 0.6;

pub(super) struct Seat {
    pub(super) widget: WidgetId,
    pub(super) rect: Rect,
    pub(super) transition_style: TransitionStyle,
    pub(super) leaving: bool,
    pub(super) grabbed: bool,
    pub(super) touch: Option<Touch>,
}

#[derive(Debug, Default)]
pub(super) struct SeatState {
    target: Option<Rect>,
    gliding: TransitionStyle,
    shift: Point,
    pressing: Pressing,
}

impl Seat {
    pub(super) fn holding(self, content: Element) -> Element {
        Element::with_children(self, vec![content])
    }

    fn place(&self, state: &mut SeatState, ctx: &mut PaintCtx<'_>) -> Rect {
        let fresh = state.target.is_none();
        if state.target != Some(self.rect) {
            state.target = Some(self.rect);
            state.gliding = self.transition_style;
            if fresh || !self.transition_style.arrives() {
                ctx.jump(PLACE, self.rect);
            }
        }
        if self.grabbed {
            ctx.jump(PLACE, self.rect);
        }
        ctx.animate_curve(PLACE, self.rect, state.gliding.glide())
    }

    fn presence(&self, fresh: bool, ctx: &mut PaintCtx<'_>) -> f32 {
        if fresh && !self.transition_style.arrives() {
            ctx.jump(PRESENT, 1.0);
        }
        ctx.progress(
            PRESENT,
            !self.leaving,
            self.transition_style.arrive(),
            self.transition_style.leave(),
        )
        .clamp(0.0, 1.0)
    }
}

impl Widget for Seat {
    type State = SeatState;

    fn layout(&self) -> Layout {
        Layout::DEFAULT
            .size(
                Length::px(self.rect.width()),
                Length::px(self.rect.height()),
            )
            .placed_at(self.rect.origin)
    }

    fn paint(&self, state: &mut SeatState, ctx: &mut PaintCtx<'_>) {
        let fresh = state.target.is_none();
        let shown = self.place(state, ctx);
        let present = self.presence(fresh, ctx);
        let grown = SHRUNK + (1.0 - SHRUNK) * present;
        let drawn = Rect::from_center(
            shown.center(),
            Size::new(shown.width() * grown, shown.height() * grown),
        );
        let desk = ctx.bounds().origin - self.rect.origin;
        state.shift = fit(ctx, self.rect.size, drawn.translate(desk));
        if present < 1.0 {
            ctx.set_subtree_opacity(present);
        }
    }

    fn children_offset(&self, state: &SeatState, _bounds: Rect, _content: Size) -> Point {
        state.shift
    }

    fn event(&self, state: &mut SeatState, event: &Event, ctx: &mut EventCtx<'_>) {
        if let Some(touch) = &self.touch {
            state
                .pressing
                .handle_touch(touch, Some(self.widget), event, ctx);
        }
    }

    fn same(&self, other: &Seat) -> bool {
        self.widget == other.widget
            && self.rect == other.rect
            && self.transition_style == other.transition_style
            && self.leaving == other.leaving
            && self.grabbed == other.grabbed
            && self.touch.as_ref().map(|touch| &touch.screen)
                == other.touch.as_ref().map(|touch| &touch.screen)
    }
}

impl std::fmt::Debug for Seat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Seat")
            .field("widget", &self.widget)
            .field("rect", &self.rect)
            .field("leaving", &self.leaving)
            .finish_non_exhaustive()
    }
}
