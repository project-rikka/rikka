use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Key, Layout, Length, PaintCtx, Point,
    PointerButton, Quad, Rect, Widget,
};
use rikka_widgets::{Grab, GrabStep};

const SIDE: f32 = 56.0;
const KNOB: f32 = 8.0;
const DOT: f32 = 2.0;
const DOT_SPACING: f32 = 2.5;
const NUDGE: f32 = 1.0;
const LARGE_NUDGE: f32 = 15.0;

#[derive(Widget)]
pub(crate) struct Dial {
    #[debug]
    degrees: f32,
    folded: bool,
    on_change: Action<(f32,)>,
    on_commit: Action<(f32,)>,
}

impl Dial {
    pub(crate) fn new<M, N>(
        degrees: f32,
        on_change: impl IntoHandler<(f32,), (), M>,
        on_commit: impl IntoHandler<(f32,), (), N>,
    ) -> Dial {
        Dial {
            degrees,
            folded: false,
            on_change: Action::new(on_change),
            on_commit: Action::new(on_commit),
        }
    }

    pub(crate) fn folded(mut self) -> Dial {
        self.folded = true;
        self
    }

    fn settled(&self, degrees: f32) -> f32 {
        let turned = (degrees + 180.0).rem_euclid(360.0) - 180.0;
        match self.folded {
            false => turned,
            true if turned > 90.0 => turned - 180.0,
            true if turned < -90.0 => turned + 180.0,
            true => turned,
        }
    }

    fn pointed(&self, bounds: Rect, position: Point) -> f32 {
        let center = bounds.center();
        let degrees = (center.y - position.y)
            .atan2(position.x - center.x)
            .to_degrees();
        self.settled(degrees.round())
    }
}

impl Widget for Dial {
    type State = Grab;

    fn layout(&self) -> Layout {
        Layout::column()
            .size(Length::px(SIDE), Length::px(SIDE))
            .fixed()
    }

    fn paint(&self, _state: &mut Grab, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let bounds = ctx.bounds();
        let center = bounds.center();
        let radius = bounds.width().min(bounds.height()) / 2.0 - 1.0;
        let edge = match ctx.focused() {
            true => palette.accent,
            false => palette.outline,
        };
        ctx.draw_quad(
            Quad::new(Rect::from_circle(center, radius))
                .fill(palette.surface_variant)
                .stroke(1.0, edge)
                .radius(radius),
        );
        let (sin, cos) = self.degrees.to_radians().sin_cos();
        let reach = radius - KNOB / 2.0 - 1.0;
        let mut along = 0.0;
        while along < reach {
            ctx.draw_circle(
                Point::new(center.x + cos * along, center.y - sin * along),
                DOT,
                palette.text,
            );
            along += DOT_SPACING;
        }
        ctx.draw_circle(
            Point::new(center.x + cos * reach, center.y - sin * reach),
            KNOB,
            palette.accent,
        );
    }

    fn event(&self, grab: &mut Grab, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match grab.update(event, ctx) {
            Some(GrabStep::Moved { to, .. }) => {
                return ctx.act(&self.on_change, (self.pointed(bounds, to),));
            }
            Some(GrabStep::Ended { at, .. }) => {
                return ctx.act(&self.on_commit, (self.pointed(bounds, at),));
            }
            Some(GrabStep::Cancelled { .. }) => return ctx.act(&self.on_commit, (self.degrees,)),
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
                ctx.act(&self.on_change, (self.pointed(bounds, position),));
            }
            Event::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                let step = match modifiers.shift {
                    true => LARGE_NUDGE,
                    false => NUDGE,
                };
                let degrees = match key {
                    Key::ArrowUp | Key::ArrowLeft => self.degrees + step,
                    Key::ArrowDown | Key::ArrowRight => self.degrees - step,
                    _ => return,
                };
                ctx.set_handled();
                ctx.act(&self.on_commit, (self.settled(degrees),));
            }
            _ => {}
        }
    }

    fn same(&self, other: &Dial) -> bool {
        self.degrees == other.degrees
            && self.folded == other.folded
            && self.on_change == other.on_change
            && self.on_commit == other.on_commit
    }

    fn behavior(&self, _state: &Grab) -> Behavior {
        Behavior::FOCUSABLE
    }
}
