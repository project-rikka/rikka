use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Key, Layout, Length, PaintCtx, Point,
    PointerButton, Quad, Rect, Widget,
};
use rikka_widgets::{Grab, GrabStep};

const SIDE: f32 = 112.0;
const MARK: f32 = 10.0;
const NUDGE: f32 = 0.01;
const LARGE_NUDGE: f32 = 0.1;
const GUIDE_ALPHA: f32 = 0.5;

#[derive(Widget)]
pub(crate) struct Pad {
    #[debug]
    at: (f32, f32),
    on_change: Action<((f32, f32),)>,
    on_commit: Action<((f32, f32),)>,
}

impl Pad {
    pub(crate) fn new<M, N>(
        at: (f32, f32),
        on_change: impl IntoHandler<((f32, f32),), (), M>,
        on_commit: impl IntoHandler<((f32, f32),), (), N>,
    ) -> Pad {
        Pad {
            at,
            on_change: Action::new(on_change),
            on_commit: Action::new(on_commit),
        }
    }

    fn square(bounds: Rect) -> Rect {
        let side = bounds.width().min(bounds.height());
        Rect::new(bounds.x(), bounds.y(), side, side)
    }

    fn pointed(bounds: Rect, position: Point) -> (f32, f32) {
        let square = Pad::square(bounds);
        (
            ((position.x - square.x()) / square.width()).clamp(0.0, 1.0),
            ((position.y - square.y()) / square.height()).clamp(0.0, 1.0),
        )
    }
}

impl Widget for Pad {
    type State = Grab;

    fn layout(&self) -> Layout {
        Layout::column()
            .size(Length::px(SIDE), Length::px(SIDE))
            .fixed()
    }

    fn paint(&self, _state: &mut Grab, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let square = Pad::square(ctx.bounds());
        let edge = match ctx.focused() {
            true => palette.accent,
            false => palette.outline,
        };
        ctx.draw_quad(
            Quad::new(square)
                .fill(palette.surface_variant)
                .stroke(1.0, edge),
        );
        let x = square.x() + self.at.0 * square.width();
        let y = square.y() + self.at.1 * square.height();
        let guide = palette.text_muted.scale_alpha(GUIDE_ALPHA);
        ctx.draw_quad(Quad::new(Rect::new(x - 0.5, square.y(), 1.0, square.height())).fill(guide));
        ctx.draw_quad(Quad::new(Rect::new(square.x(), y - 0.5, square.width(), 1.0)).fill(guide));
        ctx.draw_quad(
            Quad::new(Rect::from_circle(Point::new(x, y), MARK / 2.0))
                .fill(palette.surface)
                .stroke(2.0, palette.accent)
                .radius(MARK / 2.0),
        );
    }

    fn event(&self, grab: &mut Grab, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match grab.update(event, ctx) {
            Some(GrabStep::Moved { to, .. }) => {
                return ctx.act(&self.on_change, (Pad::pointed(bounds, to),));
            }
            Some(GrabStep::Ended { at, .. }) => {
                return ctx.act(&self.on_commit, (Pad::pointed(bounds, at),));
            }
            Some(GrabStep::Cancelled { .. }) => return ctx.act(&self.on_commit, (self.at,)),
            None => {}
        }
        match *event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                grab.begin((), ctx);
                ctx.request_focus();
                ctx.act(&self.on_change, (Pad::pointed(bounds, position),));
            }
            Event::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                let step = match modifiers.shift {
                    true => LARGE_NUDGE,
                    false => NUDGE,
                };
                let (x, y) = self.at;
                let at = match key {
                    Key::ArrowLeft => (x - step, y),
                    Key::ArrowRight => (x + step, y),
                    Key::ArrowUp => (x, y - step),
                    Key::ArrowDown => (x, y + step),
                    _ => return,
                };
                ctx.set_handled();
                ctx.act(
                    &self.on_commit,
                    ((at.0.clamp(0.0, 1.0), at.1.clamp(0.0, 1.0)),),
                );
            }
            _ => {}
        }
    }

    fn same(&self, other: &Pad) -> bool {
        self.at == other.at
            && self.on_change == other.on_change
            && self.on_commit == other.on_commit
    }

    fn behavior(&self, _state: &Grab) -> Behavior {
        Behavior::FOCUSABLE
    }
}
